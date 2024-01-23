# PostGIS Server

## Motivation

Aircraft routing and automated air traffic control (ATC) is performed with the help of a PostGIS database.

In the current implementation, route nodes are populated from a list of vertiports and aviation waypoints. Reports from `svc-compliance` will be used to build zone geometries (either permanent or temporary). 

## Upcoming

Routes that overlap with zone geometries at a customer's proposed flight time are discarded.

A shortest path algorithm (A*) can then connect the departure vertiport to the arrival vertiport. The list of waypoints is used to estimate a travel distance, duration, and (in coming releases) battery discharge.

These routes are planned to be expanded to include multiple vertically stacked flight corridors, each with a pair of separated lanes for travel in opposing directions. Such corridors are mentioned by the FAA UAM CONOPS v2.0 (figures 4, 5, 6, and 7).

Additionally, these corridors may be assigned a max capacity. Corridors at max capacity would be disregarded by the shortest path algorithm. This also allows for load balancing, prioritizing corridors with less utilization to spread traffic evenly.

## Initialization

The `scripts/postgis-init.sh` script creates the root CA and client certificate and key. These should not be used in production.

The `scripts/init.sql` script is launched automatically when the `postgis` container is started (see `docker-compose.yml`).

Use `docker compose down --volumes` to delete the local `postgis-ssl` and `postgis-data` volumes if changes have been made to either of these scripts.

## PostgreSQL Tables

The `arrow` schema defines the following tables:

| Table | Description | 
| ---- | ---- |
| [`waypoints`](#waypoints) | This table lists waypoints through which aircraft can route.
| [`vertiports`](#vertiports) | This table lists waypoints through which aircraft can route.
| [`aircraft`](#aircraft) | This table tracks aircraft locations.
| [`zones`](#zones) | This table lists zones. These can be temporary or permanent. They can be vertiports who shouldn't be flown over unless they are the destination or departure port, or controlled or restricted airspace. |

### `waypoints`

| Column | Type | Description |
| ---- | ---- | --- | 
| id | SERIAL | Unique integer identifier of the node, required for pgRouting. |
| identifier | VARCHAR UNIQUE | A unique identifier for this waypoint (e.g. 'BANANA') |
| geom | GEOMETRY(POINT) | The 2D geometry of the waypoint (no height information) |

### `vertiports`

| Column | Type | Description |
| ---- | ---- | --- | 
| id | SERIAL | Unique integer identifier of the node, required for pgRouting. |
| identifier | VARCHAR UNIQUE | A unique identifier for this vertiport. |
| label | VARCHAR UNIQUE | A display name for the vertiport. |
| geom | GEOMETRY(POLYGONZ) | The polygonal geometry of a vertiport at a specific altitude in meters. | 
| altitude_meters | FLOAT(4) | The altitude of this vertiport. |
| last_updated | TIMESTAMPTZ | The most recent timestamp of an update to this row. |
| zone_id | INTEGER FK(arrow.zones)  | The ID of the entry in the zones table associated with this vertiport.

### `aircraft`

| Column | Type | Description |
| ---- | ---- | --- | 
| id | SERIAL | Unique integer identifier of the node, required for pgRouting. |
| identifier | VARCHAR UNIQUE | A unique identifier for this aircraft. |
| aircraft_type | ENUM | The type of aircraft (e.g. Rotorcraft) | 
| geom | GEOMETRY(POINTZ) | The latitude, longitude, and altitude (in meters) of this aircraft.
| track_angle_degrees | FLOAT(4)| The heading/yaw of this aircraft with respect to true North.
| velocity_horizontal_ground_mps | FLOAT(4)| The ground speed (in meters per second) for this aircraft
| velocity_vertical_mps | FLOAT(4)| The vertical rate (in meters per second) of this aircraft.
| last_identifier_update | TIMESTAMPTZ | The time of the last telemetry report containing identifier data.
| last_position_update | TIMESTAMPTZ | The time of the last telemetry report containing position data.
| last_velocity_update | TIMESTAMPTZ | The time of the last telemetry report containing velocity data.

## `zones`

| Column | Type | Description |
| ---- | ---- | --- | 
| id | SERIAL | Unique integer identifier of the node, required for pgRouting. |
| identifier | VARCHAR | The NOTAM identifier or other unique identifier to this zone.
| zone_type | ENUM | The type of zone (e.g. Restricted)
| geom | POLYHEDRALSURFACEZ | A 3D volume indicating the boundaries and z-limits of the zone.
| altitude_meters_min | FLOAT(4) | For convenience, the minimum altitude where this zone begins.
| altitude_meters_max | FLOAT(4) | For convenience, the maximum altitude where this zone ends.
| time_start | TIMESTAMPTZ | The time that this zone becomes active. NULL if active by default, starting the moment it is created.
| time_end | TIMESTAMPTZ | The time that this zone becomes inactive. NULL if no scheduled end date.
| last_updated | TIMESTAMPTZ | The timestamp of the most recent update to this row.
