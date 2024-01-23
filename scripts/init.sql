CREATE USER svc_gis;
CREATE DATABASE gis;
\c gis

REVOKE ALL ON SCHEMA public FROM PUBLIC;

CREATE SCHEMA IF NOT EXISTS arrow;
CREATE EXTENSION postgis CASCADE;
UPDATE pg_extension
    SET extrelocatable = TRUE
    WHERE extname = 'postgis';

ALTER EXTENSION postgis
    SET SCHEMA arrow;

CREATE EXTENSION postgis_sfcgal SCHEMA arrow;

SET search_path TO "$user", arrow, postgis, topology, public;
ALTER ROLE svc_gis SET search_path TO "$user", arrow, postgis, topology, public;

GRANT ALL PRIVILEGES ON SCHEMA arrow TO svc_gis;
