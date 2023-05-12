# Concept of Operations - `svc-gis`

<center>

<img src="https://github.com/Arrow-air/tf-github/raw/main/src/templates/doc-banner-services.png" style="height:250px" />

</center>

Attribute | Description
--- | ---
Maintainer | [@Arrow-air/services](https://github.com/orgs/Arrow-air/teams)
Status | Draft

## Overview

This microservice is the abstraction layer for the PostGIS database.

It provides a limited interface for other microservices to perform specific actions within PostGIS. It provides a level of safety as it prevents other microservices from making SQL calls directly to the database.

## Related Documents

Document | Description
--- | ---
[High-Level Concept of Operations (CONOPS)](https://github.com/Arrow-air/se-services/blob/develop/docs/conops.md) | Overview of Arrow microservices.
[High-Level Interface Control Document (ICD)](https://github.com/Arrow-air/se-services/blob/develop/docs/icd.md)  | Interfaces and frameworks common to all Arrow microservices.
[Requirements - `svc-gis`](https://nocodb.arrowair.com/dashboard/#/nc/view/5a893886-20f3-41f6-af95-6a235ca52647) | Requirements and user stories for this microservice.
[Interface Control Document (ICD) - `svc-gis`](./icd.md) | Defines the inputs and outputs of this microservice.
[Software Design Document (SDD) - `svc-gis`](./sdd.md) | Specifies the internal activity of this microservice.

## Motivation

PostGIS is used to determine the intersection of geometries such as flight paths and no-fly zones. It is also used for shortest-path algorithms and max-flow algorithms.

## Needs, Goals and Objectives of Envisioned System

## Overview of System and Key Elements

## External Interfaces
See the ICD for this microservice.

## Proposed Capabilities

## Modes of Operation

## Operational Scenarios, Use Cases and/or Design Reference Missions

## Nominal & Off-Nominal Conditions

## Physical Environment

See the High-Level CONOPS.

## Support Environment

See the High-Level CONOPS.

## Impact Considerations

## Environmental Impacts

## Organizational Impacts

## Technical Impacts

## Risks and Potential Issues

## Appendix A: Citations

## Appendix B: Acronyms & Glossary
