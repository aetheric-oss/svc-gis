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

CREATE TABLE arrow.nofly (
    id SERIAL PRIMARY KEY,
    label VARCHAR(255) UNIQUE NOT NULL,
    geom GEOMETRY NOT NULL,
    time_start TIMESTAMPTZ,
    time_end TIMESTAMPTZ,
    vertiport_id uuid,
    CONSTRAINT fk_vertiport_id
        FOREIGN KEY (vertiport_id)
        REFERENCES arrow.rnodes(arrow_id)
);

CREATE INDEX nofly_idx
    ON arrow.nofly
    USING GIST (geom);

-- These permissions must be declared last
GRANT ALL ON SCHEMA arrow TO svc_gis;
GRANT ALL
    ON ALL TABLES IN SCHEMA arrow
    TO svc_gis;
GRANT ALL
    ON ALL SEQUENCES IN SCHEMA arrow
    TO svc_gis;
