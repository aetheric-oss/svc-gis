CREATE DATABASE gis;
CREATE USER svc_gis;
GRANT ALL PRIVILEGES ON DATABASE gis TO svc_gis;
\c gis

CREATE EXTENSION postgis;
CREATE EXTENSION pgrouting CASCADE;
CREATE SCHEMA arrow AUTHORIZATION postgres;

-- CREATE TABLE trajectories (
--     icao SERIAL PRIMARY KEY,
--     geom GEOMETRY NOT NULL REFERENCES routes(id),
--     name VARCHAR(255) NOT NULL,
--     description VARCHAR(255) NOT NULL,
--     created_at TIMESTAMP NOT NULL DEFAULT NOW(),
--     updated_at TIMESTAMP NOT NULL DEFAULT NOW()
-- );

CREATE TYPE NodeType AS ENUM ('waypoint', 'vertiport');

-- Vertiports and waypoints
CREATE TABLE arrow.rnodes (
    id SERIAL NOT NULL,
    arrow_id uuid UNIQUE PRIMARY KEY NOT NULL,
    node_type NodeType NOT NULL,
    geom GEOMETRY(Point) NOT NULL
);

CREATE TABLE arrow.routes (
    id SERIAL NOT NULL, /* Route ID */
    id_source INTEGER NOT NULL, /* SERIAL ID for rnode */
    id_target INTEGER NOT NULL, /* SERIAL ID for rnode */
    geom GEOMETRY(LineString) NOT NULL,
    distance float NOT NULL,
    PRIMARY KEY (id_source, id_target)
);

CREATE INDEX routes_idx
    ON arrow.routes
    USING GIST (geom);

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

CREATE TABLE arrow.aircraft (
    id SERIAL PRIMARY KEY,
    icao INTEGER UNIQUE NOT NULL,
    position GEOMETRY(Point) NOT NULL,
    altitude NUMERIC NOT NULL,
    heading NUMERIC NOT NULL,
    velocity NUMERIC NOT NULL,
    flashlight GEOMETRY(Polygon),
    target_position GEOMETRY(Point),
    target_altitude NUMERIC,
    last_report TIMESTAMPTZ
);

CREATE INDEX flashlight_idx
    ON arrow.aircraft
    USING GIST (flashlight);
