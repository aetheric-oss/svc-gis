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

SET search_path TO "$user", arrow, postgis, topology, public;
ALTER ROLE svc_gis SET search_path TO "$user", arrow, postgis, topology, public;

--------------------------------------------------------------------------------
-- FUNCTIONS
--------------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION arrow.centroid(geom GEOMETRY)
RETURNS GEOMETRY(Point)
AS $$
BEGIN
    RETURN ST_SetSRID(ST_Centroid(geom), 4326);
END; $$ LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = arrow, public, pg_temp;

GRANT ALL PRIVILEGES ON SCHEMA arrow TO svc_gis;

-- GRANT EXECUTE ON FUNCTION arrow.nearest_vertiports_to_aircraft(
--     VARCHAR,
--     INTEGER,
--     FLOAT
-- ) TO svc_gis;


---
--- Update Aircraft Identification
---
-- CREATE OR REPLACE FUNCTION arrow.update_aircraft_id(
--     craft_identifier VARCHAR(255),
--     craft_ua_type ua_type,
-- )
-- RETURNS VOID AS $$
-- DECLARE
--     node_id INTEGER;
-- BEGIN
--     IF craft_identifier IN (SELECT identifier FROM arrow.aircraft) THEN
--         -- Don't overwrite with older information
--         IF craft_time < (SELECT last_updated FROM arrow.aircraft WHERE identifier = craft_identifier) THEN
--             RETURN;
--         END IF;

--         SELECT air.node_id INTO node_id
--             FROM arrow.aircraft air WHERE identifier = craft_identifier;

--         UPDATE arrow.aircraft
--             SET ua_type = craft_ua_type
--             WHERE identifier = craft_identifier;
--         RETURN;
--     END IF;

--     INSERT INTO arrow.aircraft (
--         node_id,
--         identifier,
--         ua_type
--     ) VALUES (
--         node_id,
--         craft_identifier,
--         craft_ua_type
--     );
-- END; $$ LANGUAGE plpgsql
-- SECURITY DEFINER
-- SET search_path = arrow, pg_temp;

-- ---
-- --- Update Aircraft Position
-- ---
-- CREATE OR REPLACE FUNCTION arrow.update_aircraft_position(
--     craft_identifier VARCHAR(255),
--     craft_geom GEOMETRY(Point),
--     craft_altitude_m FLOAT,
--     craft_time TIMESTAMPTZ
-- )
-- RETURNS VOID AS $$
-- DECLARE
--     node_id INTEGER;
-- BEGIN
--     IF craft_identifier IN (SELECT identifier FROM arrow.aircraft) THEN
--         -- Don't overwrite with older information
--         IF craft_time < (SELECT last_updated FROM arrow.aircraft WHERE identifier = craft_identifier) THEN
--             RETURN;
--         END IF;

--         SELECT air.node_id INTO node_id
--             FROM arrow.aircraft air WHERE identifier = craft_identifier;

--         UPDATE arrow.nodes
--             SET geom = craft_geom WHERE id = node_id;

--         UPDATE arrow.aircraft
--             SET altitude_meters = craft_altitude_m,
--                 last_updated = craft_time,
--             WHERE identifier = craft_identifier;

--         RETURN;
--     END IF;

--     -- Vertiports are both nodes and nofly zones
--     -- The Nodes and No-Fly Zones should be created first
--     INSERT INTO arrow.nodes (node_type, geom)
--         VALUES ('Aircraft', craft_geom)
--         RETURNING id INTO node_id; -- FK constraint will fail if this fails

--     INSERT INTO arrow.aircraft (
--         node_id,
--         identifier,
--         altitude_meters,
--         last_updated
--     ) VALUES (
--         node_id,
--         craft_identifier,
--         craft_altitude_m,
--         craft_time
--     );
-- END; $$ LANGUAGE plpgsql
-- SECURITY DEFINER
-- SET search_path = arrow, pg_temp;


-- ---
-- --- Update Aircraft Velocity
-- ---
-- CREATE OR REPLACE FUNCTION arrow.update_aircraft_velocity(
--     craft_identifier VARCHAR(255),
--     craft_velocity_horizontal_ground_mps FLOAT,
--     craft_velocity_vertical_mps FLOAT,
--     craft_track_angle_degrees FLOAT,
--     craft_time TIMESTAMPTZ
-- )
-- RETURNS VOID AS $$
-- DECLARE
--     node_id INTEGER;
-- BEGIN
--     IF craft_identifier IN (SELECT identifier FROM arrow.aircraft) THEN
--         -- Don't overwrite with older information
--         IF craft_time < (SELECT last_updated FROM arrow.aircraft WHERE identifier = craft_identifier) THEN
--             RETURN;
--         END IF;

--         SELECT air.node_id INTO node_id
--             FROM arrow.aircraft air WHERE identifier = craft_identifier;

--         UPDATE arrow.aircraft
--             SET velocity_horizontal_ground_mps = craft_velocity_horizontal_ground_mps,
--                 velocity_vertical_mps = craft_velocity_vertical_mps,
--                 track_angle_degrees = craft_track_angle_degrees,
--                 last_velocity_update = craft_time
--             WHERE identifier = craft_identifier;
--         RETURN;
--     END IF;

--     INSERT INTO arrow.aircraft (
--         node_id,
--         identifier,
--         velocity_horizontal_ground_mps,
--         velocity_vertical_mps,
--         track_angle_degrees,
--         last_velocity_update
--     ) VALUES (
--         node_id,
--         craft_identifier,
--         craft_velocity_horizontal_ground_mps,
--         craft_velocity_vertical_mps,
--         craft_track_angle_degrees,
--         craft_time
--     );
-- END; $$ LANGUAGE plpgsql
-- SECURITY DEFINER
-- SET search_path = arrow, pg_temp;

-- --------------------------------------------------------------------------------
-- -- TRIGGERS
-- --------------------------------------------------------------------------------
-- -- \i triggers.sql

-- -- When a node is updated, the associated routes must be updated
-- --  as well. This trigger will update the routes table with the
-- --  new geometry and distance.
-- CREATE OR REPLACE FUNCTION arrow.route_update()
--     RETURNS TRIGGER
-- AS $$
-- BEGIN
--     -- Don't update routes for aircraft
--     IF NEW.node_type = 'Aircraft' THEN
--         RETURN NEW;
--     END IF;

--     INSERT INTO arrow.routes (
--         id_source,
--         id_target,
--         geom,
--         distance_meters
--     )
--     SELECT
--         start_node.id,
--         end_node.id,
--         ST_MakeLine(start_node.geom, end_node.geom),
--         ST_DistanceSphere(start_node.geom, end_node.geom)
--     FROM arrow.nodes start_node
--     INNER JOIN arrow.nodes end_node ON
--         (NEW.id IN (start_node.id, end_node.id))
--         AND (start_node.id <> end_node.id) -- Build two unidirectional routes
--         AND (start_node.node_type <> 'Aircraft') -- Don't route from aircraft
--         AND (end_node.node_type <> 'Aircraft') -- Don't route to aircraft
--     ON CONFLICT (id_source, id_target) DO
--         UPDATE SET geom = EXCLUDED.geom,
--             distance_meters = EXCLUDED.distance_meters;

--     RETURN NEW;
-- END;
-- $$ LANGUAGE plpgsql
-- SECURITY DEFINER
-- SET search_path = arrow, public, pg_temp;
-- CREATE TRIGGER route_update
--     AFTER UPDATE OR INSERT
--     ON arrow.nodes
--     FOR EACH ROW
--     EXECUTE PROCEDURE arrow.route_update();

-- -- When a node is deleted, the associated routes must be deleted first,
-- --  as the routes table has foreign key constraints on the nodes table.
-- -- Also must delete aircraft, vertiports, and waypoints who have a
-- --  foreign key constraint on the nodes table.
-- CREATE OR REPLACE FUNCTION arrow.route_cleanup() RETURNS trigger AS $$
--     BEGIN
--         DELETE FROM arrow.routes ar
--             WHERE (OLD.id IN (ar.id_source, ar.id_target));
--             RETURN OLD;
--     END;
-- $$ LANGUAGE plpgsql;
-- CREATE TRIGGER node_delete
--     BEFORE DELETE
--     ON arrow.nodes
--     FOR EACH ROW
-- EXECUTE FUNCTION arrow.route_cleanup();

-- -- Must delete related nodes and nofly zones when a vertiport is deleted
-- CREATE OR REPLACE FUNCTION arrow.vertiport_cleanup() RETURNS trigger AS $$
--     BEGIN
--         DELETE FROM arrow.nodes WHERE (OLD.node_id = id);
--         DELETE FROM arrow.nofly WHERE (OLD.nofly_id = id);
--         RETURN NULL;
--     END;
-- $$ LANGUAGE plpgsql;
-- CREATE TRIGGER vertiport_delete
--     AFTER DELETE
--     ON arrow.vertiports
--     FOR EACH ROW
-- EXECUTE FUNCTION arrow.vertiport_cleanup();

-- CREATE OR REPLACE FUNCTION arrow.node_cleanup()
-- RETURNS TRIGGER
-- AS $$
--     BEGIN
--         DELETE FROM arrow.nodes an WHERE an.id = OLD.node_id;
--         RETURN NULL;
--     END;
-- $$ LANGUAGE plpgsql
-- SECURITY DEFINER
-- SET search_path = arrow, pg_temp;

-- -- Must delete related nodes when a waypoint is deleted
-- CREATE TRIGGER waypoint_delete
--     AFTER DELETE
--     ON arrow.waypoints
--     FOR EACH ROW
-- EXECUTE FUNCTION arrow.node_cleanup();

-- -- Must delete related nodes when an aircraft is deleted
-- CREATE TRIGGER aircraft_delete
--     AFTER DELETE
--     ON arrow.aircraft
--     FOR EACH ROW
-- EXECUTE FUNCTION arrow.node_cleanup();

-- ---------------------------------------------------------------------
-- -- Routing Algorithms
-- ---------------------------------------------------------------------
-- -- \i routing.sql

-- CREATE OR REPLACE FUNCTION arrow.available_routes (
--     allowed_nofly_id_1 INTEGER,
--     allowed_nofly_id_2 INTEGER,
--     time_start TIMESTAMPTZ,
--     time_end TIMESTAMPTZ
-- ) RETURNS TABLE (
--     id INTEGER,
--     id_source INTEGER,
--     id_target INTEGER,
--     distance_meters FLOAT,
--     geom GEOMETRY
-- ) AS $$
-- BEGIN
--     RETURN QUERY
--         SELECT ar.id, ar.id_source, ar.id_target, ar.distance_meters, ar.geom
--         FROM arrow.routes AS ar
--         LEFT JOIN
--         (
--             SELECT anf.id, anf.geom FROM arrow.nofly AS anf
--             WHERE (
--                 (anf.id IS DISTINCT FROM $1 AND anf.id IS DISTINCT FROM $2) -- No-Fly Zone is not the start or end goals
--                 AND (
--                     (anf.time_start IS NULL AND anf.time_end IS NULL) -- No-Fly Zone is permanent
--                     OR ((anf.time_start < $4) AND (anf.time_end > $3)) -- Falls Within TFR
--                 )
--             )
--         ) AS anf
--         ON ST_Intersects(ar.geom, anf.geom)
--         WHERE anf.id IS NULL;
-- END;
-- $$ LANGUAGE plpgsql
-- SECURITY DEFINER
-- SET search_path = arrow, public, pg_temp;

-- CREATE OR REPLACE FUNCTION arrow.a_star_sql_query (
--     allowed_nofly_id_1 INTEGER,
--     allowed_nofly_id_2 INTEGER,
--     start_time timestamptz,
--     end_time timestamptz
-- )
-- RETURNS text
-- AS $$
-- BEGIN
--     RETURN FORMAT('SELECT
--         available.id,
--         available.id_source AS source,
--         available.id_target AS target,
--         available.distance_meters AS cost,
--         -1 AS reverse_cost, -- unidirectional, prevents insertion
--         ST_X(ST_StartPoint(geom)) as x1,
--         ST_Y(ST_StartPoint(geom)) as y1,
--         ST_X(ST_EndPoint(geom)) as x2,
--         ST_Y(ST_EndPoint(geom)) as y2
--     FROM
--     (
--         SELECT ar.id, ar.id_source, ar.id_target, ar.distance_meters, ar.geom
--         FROM arrow.available_routes(%L, %L, %L, %L) AS ar
--     ) AS available;', $1, $2, $3, $4);
-- END;
-- $$ LANGUAGE plpgsql
-- SECURITY DEFINER
-- SET search_path = arrow, pg_temp;

-- CREATE OR REPLACE FUNCTION arrow.best_path (
--     start_node_id INTEGER,
--     end_node_id INTEGER,
--     start_nofly_id INTEGER,
--     end_nofly_id INTEGER,
--     start_time TIMESTAMPTZ,
--     end_time TIMESTAMPTZ
-- ) RETURNS TABLE (
--     path_seq INTEGER,
--     start_id INTEGER,
--     end_id INTEGER,
--     distance_meters FLOAT
-- ) AS $body$
-- BEGIN
--     RETURN QUERY
--         SELECT
--             results.seq,
--             ar.id_source,
--             ar.id_target,
--             results.cost
--         FROM arrow.routes ar
--         JOIN (
--             SELECT seq, edge, cost
--             FROM pgr_aStar(
--                 (SELECT * FROM arrow.a_star_sql_query($3, $4, $5, $6)),
--                 $1,
--                 $2,
--                 directed => true,
--                 heuristic => 2
--             )
--         ) AS results ON ar.id = results.edge;
-- END;
-- $body$ LANGUAGE plpgsql
-- SECURITY DEFINER
-- SET search_path = arrow, public, pg_temp;

-- CREATE OR REPLACE FUNCTION arrow.best_path_p2p (
--     start_node UUID, -- Vertiport
--     end_node UUID, -- Vertiport
--     start_time TIMESTAMPTZ,
--     end_time TIMESTAMPTZ
-- ) RETURNS TABLE (
--     path_seq INTEGER,
--     start_type arrow.NodeType,
--     start_latitude FLOAT,
--     start_longitude FLOAT,
--     end_type arrow.NodeType,
--     end_latitude FLOAT,
--     end_longitude FLOAT,
--     distance_meters FLOAT
-- ) AS $body$
-- DECLARE
--     start_node_id INTEGER;
--     start_nofly_id INTEGER;
--     end_node_id INTEGER;
--     end_nofly_id INTEGER;
-- BEGIN
--     --- Get Node and Nofly IDs for start and end nodes
--     SELECT node_id, nofly_id
--         INTO start_node_id, start_nofly_id
--         FROM arrow.vertiports WHERE arrow_id = $1;

--     SELECT node_id, nofly_id
--         INTO end_node_id, end_nofly_id
--         FROM arrow.vertiports WHERE arrow_id = $2;

--     --- Get best path between nodes

--     RETURN QUERY
--         SELECT
--             results.path_seq,
--             (SELECT node_type FROM arrow.nodes WHERE id = results.start_id),
--             (SELECT ST_Y(geom) FROM arrow.nodes WHERE id = results.start_id),
--             (SELECT ST_X(geom) FROM arrow.nodes WHERE id = results.start_id),
--             (SELECT node_type FROM arrow.nodes WHERE id = results.end_id),
--             (SELECT ST_Y(geom) FROM arrow.nodes WHERE id = results.end_id),
--             (SELECT ST_X(geom) FROM arrow.nodes WHERE id = results.end_id),
--             results.distance_meters
--         FROM (
--             SELECT * FROM arrow.best_path (
--                 start_node_id,
--                 end_node_id,
--                 start_nofly_id,
--                 end_nofly_id,
--                 start_time,
--                 end_time
--             )
--         ) as results;
-- END;
-- $body$ LANGUAGE plpgsql
-- SECURITY DEFINER
-- SET search_path = arrow, public, pg_temp;

-- CREATE OR REPLACE FUNCTION arrow.best_path_a2p (
--     start_label VARCHAR, -- Aircraft
--     end_node UUID, -- Vertiport
--     start_time TIMESTAMPTZ,
--     end_time TIMESTAMPTZ
-- ) RETURNS TABLE (
--     path_seq INTEGER,
--     start_type arrow.NodeType,
--     start_latitude FLOAT,
--     start_longitude FLOAT,
--     end_type arrow.NodeType,
--     end_latitude FLOAT,
--     end_longitude FLOAT,
--     distance_meters FLOAT
-- ) AS $body$
-- DECLARE
--     start_node_id INTEGER;
--     end_node_id INTEGER;
--     end_nofly_id INTEGER;
-- BEGIN
--     --- Get Node and Nofly IDs for start and end nodes
--     SELECT node_id
--         INTO start_node_id
--         FROM arrow.aircraft WHERE identifier = $1;

--     SELECT node_id, nofly_id
--         INTO end_node_id, end_nofly_id
--         FROM arrow.vertiports WHERE arrow_id = $2;

--     -- Build routes for the specific aircraft
--     INSERT INTO arrow.routes (
--         id_source,
--         id_target,
--         geom,
--         distance_meters
--     ) SELECT
--         start_node.id,
--         end_node.id,
--         ST_MakeLine(start_node.geom, end_node.geom),
--         ST_DistanceSphere(start_node.geom, end_node.geom)
--     FROM arrow.nodes start_node
--     INNER JOIN arrow.nodes end_node ON
--         (start_node_id = start_node.id) -- Unidirectional routes from aircraft
--         AND (end_node.node_type <> 'Aircraft') -- Don't route to aircraft
--     ON CONFLICT (id_source, id_target) DO
--         UPDATE SET geom = EXCLUDED.geom,
--             distance_meters = EXCLUDED.distance_meters;

--     --- Get best path between nodes
--     RETURN QUERY
--         SELECT
--             results.path_seq,
--             (SELECT node_type FROM arrow.nodes WHERE id = results.start_id),
--             (SELECT ST_Y(geom) FROM arrow.nodes WHERE id = results.start_id),
--             (SELECT ST_X(geom) FROM arrow.nodes WHERE id = results.start_id),
--             (SELECT node_type FROM arrow.nodes WHERE id = results.end_id),
--             (SELECT ST_Y(geom) FROM arrow.nodes WHERE id = results.end_id),
--             (SELECT ST_X(geom) FROM arrow.nodes WHERE id = results.end_id),
--             results.distance_meters
--         FROM (
--             SELECT * FROM arrow.best_path (
--                 start_node_id,
--                 end_node_id,
--                 NULL,
--                 end_nofly_id,
--                 start_time,
--                 end_time
--             )
--         ) as results;
-- END;
-- $body$ LANGUAGE plpgsql
-- SECURITY DEFINER
-- SET search_path = arrow, public, pg_temp;

-- -- Get the nearest vertiports to a vertiport
-- CREATE OR REPLACE FUNCTION arrow.nearest_vertiports_to_vertiport (
--     start_uuid UUID, -- Vertiport
--     n_results INTEGER,
--     max_range_meters FLOAT
-- ) RETURNS TABLE (
--     arrow_id UUID,
--     distance_meters FLOAT
-- ) AS $body$
-- DECLARE
--     start_node_id INTEGER;
-- BEGIN
--     -- Get Node ID of starting vertiport
--     SELECT node_id
--         FROM arrow.vertiports av
--         WHERE av.arrow_id = start_uuid
--         INTO start_node_id;

--     -- Plug the geometry into a nearest-neighbor query
--     RETURN QUERY
--         SELECT
--             (SELECT av.arrow_id FROM arrow.vertiports av WHERE node_id = ar.id_target),
--             ar.distance_meters
--             FROM arrow.routes ar
--             WHERE
--                 ar.id_source = start_node_id
--                 AND ar.distance_meters < max_range_meters
--                 AND (SELECT node_type FROM arrow.nodes WHERE id = ar.id_target) = 'Vertiport'
--             ORDER BY ar.distance_meters
--             LIMIT n_results;
-- END;
-- $body$ LANGUAGE plpgsql
-- SECURITY DEFINER
-- SET search_path = arrow, public, pg_temp;

-- -- Get the nearest vertiports to an aircraft
-- CREATE OR REPLACE FUNCTION arrow.nearest_vertiports_to_aircraft (
--     start_label VARCHAR, -- Aircraft
--     n_results INTEGER,
--     max_range_meters FLOAT
-- ) RETURNS TABLE (
--     arrow_id UUID,
--     distance_meters FLOAT
-- ) AS $body$
-- DECLARE
--     start_node_id INTEGER;
-- BEGIN
--     -- Get Node ID of starting vertiport
--     SELECT node_id
--         INTO start_node_id
--         FROM arrow.aircraft WHERE identifier = start_label;

--     -- Plug the geometry into a nearest-neighbor query
--     RETURN QUERY
--     SELECT
--         (SELECT av.arrow_id FROM arrow.vertiports av WHERE node_id = ar.id_target),
--         ar.distance_meters
--         FROM arrow.routes ar
--         WHERE ar.id_source = start_node_id
--             AND ar.distance_meters < max_range_meters
--             AND (SELECT node_type FROM arrow.nodes WHERE id = ar.id_target) = 'Vertiport'
--         ORDER BY ar.distance_meters
--         LIMIT n_results;
-- END;
-- $body$ LANGUAGE plpgsql
-- SECURITY DEFINER
-- SET search_path = arrow, public, pg_temp;

-- -- These permissions must be declared last
-- GRANT USAGE ON SCHEMA arrow TO svc_gis;
-- GRANT EXECUTE ON FUNCTION arrow.update_aircraft_position(
--     UUID,
--     GEOMETRY(Point),
--     FLOAT,
--     VARCHAR,
--     TIMESTAMPTZ
-- ) TO svc_gis;

-- GRANT EXECUTE ON FUNCTION arrow.update_nofly(
--     VARCHAR,
--     GEOMETRY(Polygon),
--     TIMESTAMPTZ,
--     TIMESTAMPTZ
-- ) TO svc_gis;

-- GRANT EXECUTE ON FUNCTION arrow.update_vertiport(
--     UUID,
--     GEOMETRY(Polygon),
--     VARCHAR
-- ) TO svc_gis;

-- GRANT EXECUTE ON FUNCTION arrow.update_waypoint(
--     VARCHAR,
--     GEOMETRY(Point)
-- ) TO svc_gis;

-- GRANT EXECUTE ON FUNCTION arrow.best_path_p2p(
--     UUID,
--     UUID,
--     TIMESTAMPTZ,
--     TIMESTAMPTZ
-- ) TO svc_gis;

-- GRANT EXECUTE ON FUNCTION arrow.best_path_a2p(
--     VARCHAR,
--     UUID,
--     TIMESTAMPTZ,
--     TIMESTAMPTZ
-- ) TO svc_gis;

-- GRANT EXECUTE ON FUNCTION arrow.nearest_vertiports_to_vertiport(
--     UUID,
--     INTEGER,
--     FLOAT
-- ) TO svc_gis;

-- GRANT EXECUTE ON FUNCTION arrow.nearest_vertiports_to_aircraft(
--     VARCHAR,
--     INTEGER,
--     FLOAT
-- ) TO svc_gis;
