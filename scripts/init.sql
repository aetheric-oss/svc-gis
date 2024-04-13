CREATE USER svc_gis;
CREATE DATABASE gis;
\c gis

REVOKE ALL ON SCHEMA public FROM PUBLIC;

CREATE SCHEMA IF NOT EXISTS arrow;
CREATE EXTENSION postgis CASCADE;
CREATE EXTENSION postgis_sfcgal CASCADE;

SET search_path TO "$user", arrow, postgis, topology, public;
ALTER ROLE svc_gis SET search_path TO "$user", arrow, postgis, topology, public;
GRANT ALL PRIVILEGES ON SCHEMA arrow TO svc_gis;
ALTER DATABASE gis OWNER TO svc_gis;
