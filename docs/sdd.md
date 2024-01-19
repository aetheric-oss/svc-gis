![Arrow Banner](https://github.com/Arrow-air/tf-github/raw/main/src/templates/doc-banner-services.png)

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
[High-Level Concept of Operations (CONOPS)](https://github.com/Arrow-air/se-services/blob/develop/docs/conops.md) | Overview of Arrow microservices.
[High-Level Interface Control Document (ICD)](https://github.com/Arrow-air/se-services/blob/develop/docs/icd.md)  | Interfaces and frameworks common to all Arrow microservices.
[Requirements - `svc-gis`](https://nocodb.arrowair.com/dashboard/#/nc/view/5a893886-20f3-41f6-af95-6a235ca52647) | Requirements and user stories for this microservice.
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
    participant client as svc-scheduler
    participant gis as svc-gis
    participant postgis as PostGIS

    note over client: test
```

### updateWaypoints


```mermaid
sequenceDiagram
    participant client as svc-compliance
    participant gis as svc-gis
    participant postgis as PostGIS

    note over client: receive adsb data
    client->>+gis: updateWaypoints
    note over gis: process
    alt invalid request
    gis->>+client: error
    end
    
    alt for_each waypoint
    gis->>+postgis: update_waypoint
    
    note over postgis: create or update waypoint<br>geometry
    postgis->>+gis: success or error
    end

    note over gis: any failures will roll<br>back the entire transaction
    
    gis->>client: UpdateResponse
```

### updateZones

```mermaid
sequenceDiagram
    participant client as svc-compliance
    participant gis as svc-gis
    participant postgis as PostGIS

    note over client: receive adsb data
    client->>+gis: updateZones
    note over gis: process
    alt invalid request
    gis->>+client: error
    end
    
    alt for_each no-fly zone
    gis->>+postgis: update_zones
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

    gis->>+postgis: update_aircraft_position
    
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
    participant scheduler as svc-scheduler
    participant gis as svc-gis
    participant postgis as PostGIS

    scheduler->>+gis: bestPath(...)
    note over gis: process
    alt invalid request
    gis->>+scheduler: error
    end
    alt vertiport to vertiport
    gis->>+postgis: best_path_p2p
    end

    alt aircraft to vertiport
    gis->>+postgis: best_path_a2p
    end

    note over postgis: best_path(...)

    postgis->>+gis: Array<(start coordinates, end coordinates, meter distance)>

    gis->>+scheduler: BestPathResponse
```

### nearestNeighbors

:warning: This nearest neighbor search is not used in R3 and will be reworked in R4.

```mermaid
sequenceDiagram
    participant scheduler as svc-scheduler
    participant gis as svc-gis
    participant postgis as PostGIS

    scheduler->>+gis: nearestNeighbors(...)
    note over gis: process
    alt invalid request
    gis->>+scheduler: error
    end
    alt vertiport to vertiport
    gis->>+postgis: nearest_vertiports_to_vertiport
    end

    alt aircraft to vertiport
    gis->>+postgis: nearest_vertiports_to_aircraft
    end

    note over postgis: SELECT (...) as a <br>ORDER BY a.distance_meters

    postgis->>+gis: Array<(Vertiport UUIDs)>

    gis->>+scheduler: NearestNeighborResponse
```
