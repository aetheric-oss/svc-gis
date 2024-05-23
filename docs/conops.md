![Aetheric Banner](https://github.com/aetheric-oss/.github/raw/main/assets/doc-banner.png)

# Concept of Operations - `svc-gis`

Attribute | Description
--- | ---
Maintainer | [@aetheric-oss/dev-realm](https://github.com/orgs/aetheric-oss/teams)
Status | Draft

## :telescope: Overview

This microservice is the abstraction layer for the PostGIS database.

It provides a limited interface for other microservices to perform specific actions within PostGIS. It provides a level of safety as it prevents other microservices from making SQL calls directly to the database.

## :books: Related Documents

Document | Description
--- | ---
[High-Level Concept of Operations (CONOPS)](https://github.com/aetheric-oss/se-services/blob/develop/docs/conops.md) | Overview of Aetheric microservices.
[High-Level Interface Control Document (ICD)](https://github.com/aetheric-oss/se-services/blob/develop/docs/icd.md)  | Interfaces and frameworks common to all Aetheric microservices.
[Requirements - `svc-gis`](https://nocodb.aetheric.nl/dashboard/#/nc/view/5a893886-20f3-41f6-af95-6a235ca52647) | Requirements and user stories for this microservice.
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
