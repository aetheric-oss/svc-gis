# :earth_africa: PostGIS Server

## :running: Motivation

Aircraft routing and automated air traffic control (ATC) is performed with the help of a PostGIS database.

In the current implementation, route nodes are populated from a list of vertiports and aviation waypoints. Reports from `svc-compliance` will be used to build no-fly geometries (either permanent or temporary). 

## :crystal_ball: Upcoming

These nodes will then be used to build routes. All nodes within 300 kilometers of one another will have a route. In the future, routes may be predetermined by the FAA, EASA, or other civil aviation agency (CAA).

Routes that overlap with no-fly geometries at a customer's proposed flight time are discarded.

A shortest path algorithm (A*) can then connect the departure vertiport to the arrival vertiport. The list of waypoints is used to estimate a travel distance, duration, and (in coming releases) battery discharge.

These routes are planned to be expanded to include multiple vertically stacked flight corridors, each with a pair of separated lanes for travel in opposing directions. Such corridors are mentioned by the FAA UAM CONOPS v2.0 (figures 4, 5, 6, and 7).

Additionally, these corridors can be assigned a max capacity. Corridors at max capacity would be disregarded by the shortest path algorithm. This also allows for load balancing, prioritizing corridors with less utilization to spread traffic evenly.

## Initialization

The `scripts/postgis-init.sh` script creates the root CA and client certificate and key. These should not be used in production.

The `scripts/init.sql` script is launched automatically when the `postgis` container is started (see `docker-compose.yml`).

Use `docker compose down --volumes` to delete the local `postgis-ssl` and `postgis-data` volumes if changes have been made to either of these scripts.

## :elephant: PostgreSQL Database

### :telescope: Overview

The `arrow` schema defines the following tables:

| Table | Description | 
| ---- | ---- |
| [`rnodes`](#pushpin-rnodes) | This table lists nodes through which aircraft can route.<br>Node types currently includes:<br>- Waypoints<br>- Vertiports |
| [`nofly`](#no_entry-nofly) | This table lists no-fly zones. These can be temporary or permanent. They can be vertiports who shouldn't be flown over unless they are the destination or departure port. |

### :pushpin: `rnodes`

| Column | Type | Description |
| ---- | ---- | --- | 
| id | SERIAL | Unique integer identifier of the node, required for pgRouting. |
| **arrow_id (PK)** | UUID UNIQUE | The Arrow UUID identifier of this waypoint or vertiport. |
| node_type | enum ('waypoint', 'vertiport') | The type of route node this represents. |
| geom | GEOMETRY(Point) | The latitude and longitude of this node, altitude ignored. | 

## :no_entry: `nofly`

| Column | Type | Description |
| ---- | ---- | --- | 
| id | SERIAL | Unique integer identifier of the node, required for pgRouting. |
| label | VARCHAR | The NOTAM identifier or other label unique to this no-fly zone.
| geom | GEOMETRY | The polygon defining the bounds of this no-fly zone
| time_start | TIMESTAMPTZ | The time that this no-fly zone becomes active. NULL if active by default, starting the moment it is created.
| time_end | TIMESTAMPTZ | The time that this no-fly zone becomes inactive. NULL if no scheduled end date.
| vertiport_id | UUID UNIQUE | The Arrow UUID identifier of this vertiport, if the no-fly is specifically for a vertiport. |
