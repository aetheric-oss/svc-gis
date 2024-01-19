# PostGIS Server

## Motivation

Aircraft routing and automated air traffic control (ATC) is performed with the help of a PostGIS database.

In the current implementation, route nodes are populated from a list of vertiports and aviation waypoints. Reports from `svc-compliance` will be used to build no-fly geometries (either permanent or temporary). 

## Upcoming

These nodes will then be used to build routes. All nodes within 300 kilometers of one another will have a route. In the future, routes may be predetermined by the FAA, EASA, or other civil aviation agency (CAA).

Routes that overlap with no-fly geometries at a customer's proposed flight time are discarded.

A shortest path algorithm (A*) can then connect the departure vertiport to the arrival vertiport. The list of waypoints is used to estimate a travel distance, duration, and (in coming releases) battery discharge.

These routes are planned to be expanded to include multiple vertically stacked flight corridors, each with a pair of separated lanes for travel in opposing directions. Such corridors are mentioned by the FAA UAM CONOPS v2.0 (figures 4, 5, 6, and 7).

Additionally, these corridors can be assigned a max capacity. Corridors at max capacity would be disregarded by the shortest path algorithm. This also allows for load balancing, prioritizing corridors with less utilization to spread traffic evenly.

## Initialization

The `scripts/postgis-init.sh` script creates the root CA and client certificate and key. These should not be used in production.

The `scripts/init.sql` script is launched automatically when the `postgis` container is started (see `docker-compose.yml`).

Use `docker compose down --volumes` to delete the local `postgis-ssl` and `postgis-data` volumes if changes have been made to either of these scripts.

## PostgreSQL Tables

The `arrow` schema defines the following tables:

| Table | Description | 
| ---- | ---- |
| [`nodes`](#nodes) | Nodes for shortest path algorithms.
| [`waypoints`](#waypoints) | This table lists waypoints through which aircraft can route.
| [`vertiports`](#vertiports) | This table lists waypoints through which aircraft can route.
| [`aircraft`](#aircraft) | This table tracks aircraft locations.
| [`nofly`](#nofly) | This table lists no-fly zones. These can be temporary or permanent. They can be vertiports who shouldn't be flown over unless they are the destination or departure port. |
| [`routes`](#routes) | This table lists routes between all nodes. These routes are currently auto-populated at a hardcoded altitude.

### `nodes`

| Column | Type | Description |
| ---- | ---- | --- | 
| id | SERIAL | Unique integer identifier of the node, required for pgRouting. |
| geom | GEOMETRY(Point) | The latitude and longitude of this node, altitude ignored. |
| node_type | Enum NodeType | 'vertiport', 'aircraft', 'waypoint'

### `waypoints`

| Column | Type | Description |
| ---- | ---- | --- | 
| id | SERIAL | Unique integer identifier of the node, required for pgRouting. |
| label | VARCHAR UNIQUE | A unique identifier for this waypoint (e.g. 'BANANA') |
| node_id | INTEGER FK(arrow.nodes) | The ID of the entry in the node table associated with this waypoints. | 
| min_altitude_meters | INTEGER | The starting altitude of this waypoint, in meters.

### `vertiports`

| Column | Type | Description |
| ---- | ---- | --- | 
| id | SERIAL | Unique integer identifier of the node, required for pgRouting. |
| label | VARCHAR UNIQUE | A unique identifier for this vertiport. |
| node_id | INTEGER FK(arrow.nodes) | The ID of the entry in the node table associated with this vertiport. | 
| nofly_id | INTEGER FK(arrow.nofly)  | The ID of the entry in the nofly table associated with this vertiport.
| arrow_id | UUID UNIQUE | The Arrow UUID for this vertiport.

### `aircraft`

| Column | Type | Description |
| ---- | ---- | --- | 
| id | SERIAL | Unique integer identifier of the node, required for pgRouting. |
| identifier | VARCHAR UNIQUE | A unique identifier for this aircraft. |
| node_id | INTEGER FK(arrow.nodes) | The ID of the entry in the node table associated with this aircraft. | 
| arrow_id | UUID UNIQUE | The Arrow UUID for this aircraft. 
| altitude_meters | FLOAT | The altitude of this aircraft.
| heading_radians | FLOAT | The heading/yaw of this aircraft.
| pitch_radians | FLOAT | The pitch of this aircraft.
| velocity_mps | FLOAT | The speed of this aircraft.
| last_report | TIMESTAMPTZ | The time of the last telemetry report.

## `nofly`

| Column | Type | Description |
| ---- | ---- | --- | 
| id | SERIAL | Unique integer identifier of the node, required for pgRouting. |
| label | VARCHAR | The NOTAM identifier or other label unique to this no-fly zone.
| geom | GEOMETRY(POLYGON) | The polygon defining the bounds of this no-fly zone
| time_start | TIMESTAMPTZ | The time that this no-fly zone becomes active. NULL if active by default, starting the moment it is created.
| time_end | TIMESTAMPTZ | The time that this no-fly zone becomes inactive. NULL if no scheduled end date.
| nofly_type | Enum ZoneType | 'nofly', 'vertiport'

## `routes`

Routes are currently unidirectional. There will be two routes per pair of nodes, one traveling from A -> B and one from B -> A.

| Column | Type | Description |
| ---- | ---- | --- | 
| id | SERIAL | Unique integer identifier of the route, required for pgRouting. |
| id_source | INTEGER (FK nodes.id) | The ID of of the first node.
| id_target | INTEGER (FK nodes.id) | The ID of the second node.
| geom | GEOMETRY(LineString) | The line connecting the two nodes
| distance_meters | f64 | The distance of this line segment.

## PostgreSQL Triggers

Trigger | Action | Description
--- | --- | ---
node_delete | BEFORE DELETE arrow.nodes | `routes.id_source` and `routes.id_target` have foreign constraints on the `nodes.id` field. Removes a route if the id_source or id_target matches the node being deleted.
route_update | AFTER INSERT OR UPDATE arrow.nodes | When a 'vertiport' or 'waypoint' type node is added, there should be a route added between that node and all other nodes nearby.
vertiport_delete | AFTER DELETE arrow.vertiports | After deleting a vertiport, its associated `node` used for routing is no longer needed. Likewise, its associated `nofly` zone won't exist.
waypoint_delete | AFTER DELETE arrow.waypoints | After deleting a waypoint, its associated `node` used for routing is no longer needed.
aircraft_delete | AFTER DELETE arrow.aircraft | After deleting an aircraft, its associated `node` used for routing is no longer needed.
