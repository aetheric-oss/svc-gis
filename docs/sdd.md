![Aetheric Banner](https://github.com/aetheric-oss/.github/raw/main/assets/doc-banner.png)

# Software Design Document (SDD) - `svc-gis` 

## :telescope: Overview

This document details the software implementation of svc-gis.

This service is the abstraction layer to the PostGIS database, similar to how `svc-storage` is the abstraction layer to the PostgreSQL database.

Attribute | Description
--- | ---
Status | Draft

## :books: Related Documents

Document | Description
--- | ---
[High-Level Concept of Operations (CONOPS)](https://github.com/aetheric-oss/se-services/blob/develop/docs/conops.md) | Overview of Aetheric microservices.
[High-Level Interface Control Document (ICD)](https://github.com/aetheric-oss/se-services/blob/develop/docs/icd.md)  | Interfaces and frameworks common to all Aetheric microservices.
[Requirements - `svc-gis`](https://nocodb.aetheric.nl/dashboard/#/nc/view/5a893886-20f3-41f6-af95-6a235ca52647) | Requirements and user stories for this microservice.
[Concept of Operations - `svc-gis`](./conops.md) | Defines the motivation and duties of this microservice.
[Interface Control Document (ICD) - `svc-gis`](./icd.md) | Defines the inputs and outputs of this microservice.

## :dna: Module Attributes

Attribute | Applies | Explanation
--- | --- | ---
Safety Critical | Y | This module gates access to the PostGIS database which will be used to calculate potential collisions from current aircraft positions. 

## :globe_with_meridians: Global Variables

None

## :gear: Logic

### Initialization

At initialization this service creates two servers on separate threads: a GRPC server and a REST server.

The REST server expects the following environment variables to be set:
- `DOCKER_PORT_REST` (default: `8000`)

The GRPC server expects the following environment variables to be set:
- `DOCKER_PORT_GRPC` (default: `50051`)

### Control Loop

As a REST and GRPC server, this service awaits requests and executes handlers.

Some handlers **require** the following environment variables to be set:
- PG__USER
- PG__DBNAME
- PG__HOST
- PG__PORT
- PG__SSLMODE
- DB_CA_CERT
- DB_CLIENT_CERT
- DB_CLIENT_KEY

This information allows `svc-gis` to connect to the PostgreSQL database.

### Cleanup

None

## :speech_balloon: gRPC Handlers

See [the ICD](./icd.md) for this microservice.

### updateVertiports

```mermaid
sequenceDiagram
    participant client as svc-gis-client-grpc
    participant gis as svc-gis
    participant postgis as PostGIS

    client->>+gis: updateVertiports
    note over gis: process
    alt invalid request
    gis->>+client: error
    end
    
    alt for_each vertiport
    gis->>+postgis: INSERT .. ON CONFLICT ..
    
    note over postgis: create or update vertiport<br>geometry, egress, ingress
    postgis->>+gis: success or error
    end

    note over gis: any failures will roll<br>back the entire transaction
    
    gis->>client: UpdateResponse
```

### updateWaypoints


```mermaid
sequenceDiagram
    participant client as svc-gis-client-grpc
    participant gis as svc-gis
    participant postgis as PostGIS

    client->>+gis: updateWaypoints
    note over gis: process
    alt invalid request
    gis->>+client: error
    end
    
    alt for_each waypoint
    gis->>+postgis: INSERT .. ON CONFLICT ..
    
    note over postgis: create or update waypoint<br>geometry
    postgis->>+gis: success or error
    end

    note over gis: any failures will roll<br>back the entire transaction
    
    gis->>client: UpdateResponse
```

### updateZones

```mermaid
sequenceDiagram
    participant client as svc-gis-client-grpc
    participant gis as svc-gis
    participant postgis as PostGIS

    client->>+gis: updateZones
    note over gis: process
    alt invalid request
    gis->>+client: error
    end
    
    alt for_each no-fly zone
    gis->>+postgis: INSERT .. ON CONFLICT ..
    note over postgis: create or update no fly zone<br>time window and geometry
    
    postgis->>+gis: success or error
    end

    note over gis: any errors will roll back<br>entire transaction

    gis->>client: UpdateResponse
```

### updateAircraftPosition

```mermaid
sequenceDiagram
    participant client as svc-telemetry
    participant gis as svc-gis
    participant postgis as PostGIS

    note over client: receive adsb data
    client->>+gis: updateAircraftPosition
    note over gis: process
    alt invalid request
    gis->>+client: error
    end

    gis->>+postgis: INSERT .. ON CONFLICT ..
    
    alt Aircraft does not exist
    note over postgis: add aircraft as node
    end

    alt Aircraft exists
    note over postgis: Update geom, altitude, uuid
    end
    
    postgis->>+gis: success or error
    gis->>client: UpdateResponse
```

### bestPath

```mermaid
sequenceDiagram
    participant client as svc-gis-client-grpc
    participant gis as svc-gis
    participant postgis as PostGIS

    client->>+gis: bestPath(...)
    note over gis: process
    alt invalid request
    gis->>+client: error
    end
    alt vertiport to vertiport
    gis->>+postgis: best_path_p2p
    end

    alt aircraft to vertiport
    gis->>+postgis: best_path_a2p
    end

    note over postgis: best_path(...)

    postgis->>+gis: Array<(start coordinates, end coordinates, meter distance)>

    gis->>+client: BestPathResponse
```


### checkIntersection

This compares a path against two sources of geometry: zones and existing flight paths.

Currently any overlap between a path and a zone will be considered an intersection.

A different method is taken with comparing flight paths with one another. First flight paths are compared in their entireties for intersection to narrow down the field of possible aircraft collisions.

These full path intersections aren't disqualifying, as the flights may occur at non-overlapping times or may overlap in time in such a way that the two aircraft will be unlikely to come near one another.

As an example, two identical flights between vertiports A and B may have identical flight paths. However, if they leave 3 days apart, it is improbable that a collision will occur. Likewise, for any paths that intersect at a point in space, the time at which the intersection occurs must be taken into account before declaring an intersection.

We consider an "intersection" to be any two paths that come within N meters of one another. This turns flight paths into 3D cylindrical volumes for the purposes of determining intersection.

```mermaid
sequenceDiagram
    participant client as svc-gis-client-grpc
    participant gis as svc-gis
    participant postgis as PostGIS

    client->>+gis: checkIntersection(path)
    gis->>postgis: ST_3DIntersects() path and all zones (limit 1)
    postgis->>gis: Intersections
    alt if intersection
        gis->>+client: CheckIntersectionResponse
    end
    
    gis->>postgis: ST_3DDistance() between path and existing flight paths

    loop
        postgis->>gis: intersections
        note over gis: if no overlap in time between paths<br>discard intersection
        alt ST_3DDistance(path_a, path_b) < THRESHOLD
            note over gis: these paths intersect somewhere along their duration
            note over gis: split both paths in half,<br>compare the first halves and the second halves
            gis->>postgis: ST_3DDistance(path_a_1, path_b_1) < THRESHOLD
            gis->>postgis: ST_3DDistance(path_a_2, path_b_2) < THRESHOLD
        end

        note over gis: keep splitting where the paths intersect
        note over gis: if we keep splitting until the paths are<br>less than N meters in length<br>and still find an intersection<br>the flight paths cross and the aircraft will potentially collide
        note over gis: if no more intersections are found<br>before that minimum distance is reached<br>these flights intersect but at different<br>points in time, aircraft are<br>unlikely to collide
    end

    gis->>+client: CheckIntersectionResponse
```
