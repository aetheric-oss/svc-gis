CREATE DATABASE gis;
CREATE USER svc_gis;
\c gis

CREATE SCHEMA arrow;
CREATE EXTENSION postgis;
CREATE EXTENSION pgrouting CASCADE;

CREATE TYPE NodeType AS ENUM ('waypoint', 'vertiport');

-- Vertiports and waypoints
CREATE TABLE arrow.rnodes (
    id SERIAL NOT NULL,
    arrow_id uuid UNIQUE PRIMARY KEY NOT NULL,
    node_type NodeType NOT NULL,
    geom GEOMETRY(Point) NOT NULL
);

-- These permissions must be declared last
GRANT ALL ON SCHEMA arrow TO svc_gis;
GRANT ALL
    ON ALL TABLES IN SCHEMA arrow
    TO svc_gis;
GRANT ALL
    ON ALL SEQUENCES IN SCHEMA arrow
    TO svc_gis;
