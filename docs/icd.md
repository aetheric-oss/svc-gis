# Interface Control Document (ICD) - `svc-gis`

<center>

<img src="https://github.com/Arrow-air/tf-github/raw/main/src/templates/doc-banner-services.png" style="height:250px" />

</center>

## Overview

This document defines the gRPC and REST interfaces unique to the `svc-gis` microservice.

Attribute | Description
--- | ---
Status | Draft

## Related Documents

Document | Description
--- | ---
[High-Level Concept of Operations (CONOPS)](https://github.com/Arrow-air/se-services/blob/develop/docs/conops.md) | Overview of Arrow microservices.
[High-Level Interface Control Document (ICD)](https://github.com/Arrow-air/se-services/blob/develop/docs/icd.md)  | Interfaces and frameworks common to all Arrow microservices.
[Requirements - `svc-gis`](https://nocodb.arrowair.com/dashboard/#/nc/view/5a893886-20f3-41f6-af95-6a235ca52647) | Requirements and user stories for this microservice.
[Concept of Operations - `svc-gis`](./conops.md) | Defines the motivation and duties of this microservice.
[Software Design Document (SDD) - `svc-gis`](./sdd.md) | Specifies the internal activity of this microservice.

## Frameworks

See the High-Level ICD.

## REST

This microservice implements no additional REST endpoints beyond the common REST interfaces (see High-Level ICD).

## gRPC

### Files

These interfaces are defined in a protocol buffer file, `proto/grpc.proto`.

### Integrated Authentication & Encryption

See the High-Level ICD.

### gRPC Server Methods ("Services")

| Service | Description |
| ---- | ---- |
| `GetExample` | This is an example Service.<br>Replace

### gRPC Client Messages ("Requests")

| Request | Description |
| ------    | ------- |
| `ExampleQuery` | A message to illustrate an example
