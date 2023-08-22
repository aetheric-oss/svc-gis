CREATE USER svc_gis;
CREATE DATABASE gis;
\c gis

CREATE SCHEMA arrow;
REVOKE ALL ON SCHEMA public FROM PUBLIC;
CREATE EXTENSION pgrouting CASCADE;
SET search_path TO "$user", postgis, topology, public;
ALTER ROLE svc_gis SET search_path TO "$user", postgis, topology, public;

--
-- Nodes for Path Finding
--
CREATE TYPE arrow.NodeType AS ENUM ('Waypoint', 'Vertiport', 'Aircraft');
CREATE TYPE arrow.NoFlyType AS ENUM ('Nofly', 'Vertiport');
CREATE TABLE arrow.nodes (
    id SERIAL PRIMARY KEY NOT NULL,
    node_type arrow.NodeType NOT NULL,
    geom GEOMETRY(Point) NOT NULL
);
CREATE INDEX nodes_idx
    ON arrow.nodes
    USING GIST (geom);

---
--- Edges in Path Finding
---
CREATE TABLE arrow.routes (
    id SERIAL NOT NULL,
    id_source INTEGER NOT NULL,
    id_target INTEGER NOT NULL,
    geom GEOMETRY(LineString) NOT NULL,
    distance_meters FLOAT NOT NULL,
    PRIMARY KEY (id_source, id_target),
    CONSTRAINT fk_source
        FOREIGN KEY (id_source)
        REFERENCES arrow.nodes(id),
    CONSTRAINT fk_target
        FOREIGN KEY (id_target)
        REFERENCES arrow.nodes(id)
);
CREATE INDEX routes_idx
    ON arrow.routes
    USING GIST (geom);

---
--- No Fly Zones
---
CREATE TABLE arrow.nofly (
    id SERIAL PRIMARY KEY,
    label VARCHAR(255) UNIQUE NOT NULL,
    nofly_type arrow.NoFlyType NOT NULL,
    geom GEOMETRY(Polygon) NOT NULL,
    time_start TIMESTAMPTZ,
    time_end TIMESTAMPTZ
);
CREATE INDEX nofly_idx
    ON arrow.nofly
    USING GIST (geom);

---
--- Vertiports are nodes in a path and have a no-fly zone
---  for flights not from or to this vertiport
---
CREATE TABLE arrow.vertiports (
    label VARCHAR(255) UNIQUE NOT NULL,
    node_id INTEGER UNIQUE NOT NULL,
    nofly_id INTEGER UNIQUE NOT NULL,
    arrow_id UUID UNIQUE NOT NULL PRIMARY KEY,
    CONSTRAINT fk_node_id
        FOREIGN KEY (node_id)
        REFERENCES arrow.nodes(id),
    CONSTRAINT fk_nofly
        FOREIGN KEY (nofly_id)
        REFERENCES arrow.nofly(id)
);

---
--- Waypoints are fixed columns in space through which
---  aircraft can pass.
---
CREATE TABLE arrow.waypoints (
    label VARCHAR(255) UNIQUE NOT NULL,
    node_id INTEGER NOT NULL,
    min_altitude_meters INTEGER NOT NULL DEFAULT 0, -- TODO(R4) Topography
    CONSTRAINT fk_node_id
        FOREIGN KEY (node_id)
        REFERENCES arrow.nodes(id)
);

---
--- Aircraft are moving nodes.
--- They are not included by default in path routing, only
---  if an aircraft is being routed to another node.
---
CREATE TABLE arrow.aircraft (
    node_id INTEGER UNIQUE,
    arrow_id UUID UNIQUE,
    callsign VARCHAR(255) UNIQUE NOT NULL PRIMARY KEY,
    altitude_meters FLOAT NOT NULL,
    last_updated TIMESTAMPTZ NOT NULL,
    CONSTRAINT fk_node_id
        FOREIGN KEY (node_id)
        REFERENCES arrow.nodes(id)
);

--------------------------------------------------------------------------------
-- FUNCTIONS
--------------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION arrow.centroid(geom GEOMETRY(Polygon))
RETURNS GEOMETRY(Point)
AS $$
BEGIN
    RETURN ST_SetSRID(ST_Centroid(geom), 4326);
END; $$ LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = arrow, public, pg_temp;

---
--- Add Vertiport
---
CREATE OR REPLACE FUNCTION arrow.update_vertiport(
    port_uuid UUID,
    port_geom GEOMETRY(Polygon),
    port_label VARCHAR(255)
) RETURNS VOID
AS $$
DECLARE
    node_id INTEGER;
    nofly_id INTEGER;
BEGIN
    IF port_uuid IN (SELECT v.arrow_id FROM arrow.vertiports v) THEN
        SELECT vs.node_id, vs.nofly_id INTO node_id, nofly_id
            FROM arrow.vertiports vs WHERE vs.arrow_id = port_uuid;

        UPDATE arrow.nodes
            SET geom = arrow.centroid(port_geom)
            WHERE id = node_id;

        UPDATE arrow.nofly
            SET geom = port_geom WHERE id = nofly_id;

        IF port_label IS NOT NULL THEN
            UPDATE arrow.vertiports av
                SET label = port_label WHERE arrow_id = port_uuid;
        END IF;

        RETURN;
    END IF;

    -- Vertiports are both nodes and nofly zones
    -- The Nodes and No-Fly Zones should be created first
    INSERT INTO arrow.nodes (node_type, geom)
        VALUES ('Vertiport', arrow.centroid(port_geom))
        RETURNING id INTO node_id; -- FK constraint will fail if this fails

    INSERT INTO arrow.nofly (label, geom, nofly_type, time_start, time_end)
        VALUES (port_label, port_geom, 'Vertiport', NULL, NULL)
        RETURNING id INTO nofly_id; -- FK constraint will fail if this fails

    INSERT INTO arrow.vertiports (node_id, nofly_id, arrow_id, label)
        VALUES (node_id, nofly_id, port_uuid, port_label);
END; $$ LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = arrow, pg_temp;

---
--- Add Waypoint
---
CREATE OR REPLACE FUNCTION arrow.update_waypoint(
    pt_label VARCHAR(255),
    pt_geom GEOMETRY(Point)
)
RETURNS VOID AS $$
DECLARE
    node_id INTEGER;
BEGIN
    IF pt_label IN (SELECT label FROM arrow.waypoints) THEN
        SELECT aw.node_id INTO node_id
            FROM arrow.waypoints aw WHERE label = pt_label;

        UPDATE arrow.nodes
            SET geom = pt_geom WHERE id = node_id;

        RETURN;
    END IF;

    -- Vertiports are both nodes and nofly zones
    -- The Nodes and No-Fly Zones should be created first
    INSERT INTO arrow.nodes (node_type, geom)
        VALUES ('Waypoint', pt_geom)
        RETURNING id INTO node_id; -- FK constraint will fail if this fails

    INSERT INTO arrow.waypoints (node_id, label)
        VALUES (node_id, pt_label);
END; $$ LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = arrow, pg_temp;

---
--- Update No-Fly Zone
---
CREATE OR REPLACE FUNCTION arrow.update_nofly(
    nofly_label VARCHAR(255),
    nofly_geom GEOMETRY(Polygon),
    nofly_time_start TIMESTAMPTZ,
    nofly_time_end TIMESTAMPTZ
) RETURNS VOID AS $$
BEGIN
    INSERT INTO arrow.nofly (label, geom, nofly_type, time_start, time_end)
        VALUES (nofly_label, nofly_geom, 'Nofly', nofly_time_start, nofly_time_end)
        ON CONFLICT(label)
            DO UPDATE
                SET geom = EXCLUDED.geom,
                    time_start = EXCLUDED.time_start,
                    time_end = EXCLUDED.time_end;
END; $$ LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = arrow, pg_temp;

---
--- Update Aircraft Position
---
CREATE OR REPLACE FUNCTION arrow.update_aircraft_position(
    craft_uuid UUID,
    craft_geom GEOMETRY(Point),
    craft_altitude_m FLOAT,
    craft_callsign VARCHAR(255),
    craft_time TIMESTAMPTZ
)
RETURNS VOID AS $$
DECLARE
    node_id INTEGER;
BEGIN
    IF craft_callsign IN (SELECT callsign FROM arrow.aircraft) THEN
        -- Don't overwrite with older information
        IF craft_time < (SELECT last_updated FROM arrow.aircraft WHERE callsign = craft_callsign) THEN
            RETURN;
        END IF;

        SELECT air.node_id INTO node_id
            FROM arrow.aircraft air WHERE callsign = craft_callsign;

        UPDATE arrow.nodes
            SET geom = craft_geom WHERE id = node_id;

        UPDATE arrow.aircraft
            SET altitude_meters = craft_altitude_m,
                last_updated = craft_time,
                arrow_id = craft_uuid
            WHERE callsign = craft_callsign;

        RETURN;
    END IF;

    -- Vertiports are both nodes and nofly zones
    -- The Nodes and No-Fly Zones should be created first
    INSERT INTO arrow.nodes (node_type, geom)
        VALUES ('Aircraft', craft_geom)
        RETURNING id INTO node_id; -- FK constraint will fail if this fails

    INSERT INTO arrow.aircraft (
        node_id,
        arrow_id,
        callsign,
        altitude_meters,
        last_updated
    ) VALUES (
        node_id,
        craft_uuid,
        craft_callsign,
        craft_altitude_m,
        craft_time
    );
END; $$ LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = arrow, pg_temp;

--------------------------------------------------------------------------------
-- TRIGGERS
--------------------------------------------------------------------------------
-- \i triggers.sql

-- When a node is updated, the associated routes must be updated
--  as well. This trigger will update the routes table with the
--  new geometry and distance.
CREATE OR REPLACE FUNCTION arrow.route_update()
    RETURNS TRIGGER
AS $$
BEGIN
    -- Don't update routes for aircraft
    IF NEW.node_type = 'Aircraft' THEN
        RETURN NEW;
    END IF;

    INSERT INTO arrow.routes (
        id_source,
        id_target,
        geom,
        distance_meters
    )
    SELECT
        start_node.id,
        end_node.id,
        ST_MakeLine(start_node.geom, end_node.geom),
        ST_DistanceSphere(start_node.geom, end_node.geom)
    FROM arrow.nodes start_node
    INNER JOIN arrow.nodes end_node ON
        (NEW.id IN (start_node.id, end_node.id))
        AND (start_node.id <> end_node.id) -- Build two unidirectional routes
        AND (start_node.node_type <> 'Aircraft') -- Don't route from aircraft
        AND (end_node.node_type <> 'Aircraft') -- Don't route to aircraft
    ON CONFLICT (id_source, id_target) DO
        UPDATE SET geom = EXCLUDED.geom,
            distance_meters = EXCLUDED.distance_meters;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = arrow, public, pg_temp;
CREATE TRIGGER route_update
    AFTER UPDATE OR INSERT
    ON arrow.nodes
    FOR EACH ROW
    EXECUTE PROCEDURE arrow.route_update();

-- When a node is deleted, the associated routes must be deleted first,
--  as the routes table has foreign key constraints on the nodes table.
-- Also must delete aircraft, vertiports, and waypoints who have a
--  foreign key constraint on the nodes table.
CREATE OR REPLACE FUNCTION arrow.route_cleanup() RETURNS trigger AS $$
    BEGIN
        DELETE FROM arrow.routes ar
            WHERE (OLD.id IN (ar.id_source, ar.id_target));
            RETURN OLD;
    END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER node_delete
    BEFORE DELETE
    ON arrow.nodes
    FOR EACH ROW
EXECUTE FUNCTION arrow.route_cleanup();

-- Must delete related nodes and nofly zones when a vertiport is deleted
CREATE OR REPLACE FUNCTION arrow.vertiport_cleanup() RETURNS trigger AS $$
    BEGIN
        DELETE FROM arrow.nodes WHERE (OLD.node_id = id);
        DELETE FROM arrow.nofly WHERE (OLD.nofly_id = id);
        RETURN NULL;
    END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER vertiport_delete
    AFTER DELETE
    ON arrow.vertiports
    FOR EACH ROW
EXECUTE FUNCTION arrow.vertiport_cleanup();

CREATE OR REPLACE FUNCTION arrow.node_cleanup()
RETURNS TRIGGER
AS $$
    BEGIN
        DELETE FROM arrow.nodes an WHERE an.id = OLD.node_id;
        RETURN NULL;
    END;
$$ LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = arrow, pg_temp;

-- Must delete related nodes when a waypoint is deleted
CREATE TRIGGER waypoint_delete
    AFTER DELETE
    ON arrow.waypoints
    FOR EACH ROW
EXECUTE FUNCTION arrow.node_cleanup();

-- Must delete related nodes when an aircraft is deleted
CREATE TRIGGER aircraft_delete
    AFTER DELETE
    ON arrow.aircraft
    FOR EACH ROW
EXECUTE FUNCTION arrow.node_cleanup();

---------------------------------------------------------------------
-- Routing Algorithms
---------------------------------------------------------------------
-- \i routing.sql

CREATE OR REPLACE FUNCTION arrow.available_routes (
    allowed_nofly_id_1 INTEGER,
    allowed_nofly_id_2 INTEGER,
    time_start TIMESTAMPTZ,
    time_end TIMESTAMPTZ
) RETURNS TABLE (
    id INTEGER,
    id_source INTEGER,
    id_target INTEGER,
    distance_meters FLOAT,
    geom GEOMETRY
) AS $$
BEGIN
    RETURN QUERY
        SELECT ar.id, ar.id_source, ar.id_target, ar.distance_meters, ar.geom
        FROM arrow.routes AS ar
        LEFT JOIN
        (
            SELECT anf.id, anf.geom FROM arrow.nofly AS anf
            WHERE (
                (anf.id IS DISTINCT FROM $1 AND anf.id IS DISTINCT FROM $2) -- No-Fly Zone is not the start or end goals
                AND (
                    (anf.time_start IS NULL AND anf.time_end IS NULL) -- No-Fly Zone is permanent
                    OR ((anf.time_start < $4) AND (anf.time_end > $3)) -- Falls Within TFR
                )
            )
        ) AS anf
        ON ST_Intersects(ar.geom, anf.geom)
        WHERE anf.id IS NULL;
END;
$$ LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = arrow, public, pg_temp;

CREATE OR REPLACE FUNCTION arrow.a_star_sql_query (
    allowed_nofly_id_1 INTEGER,
    allowed_nofly_id_2 INTEGER,
    start_time timestamptz,
    end_time timestamptz
)
RETURNS text
AS $$
BEGIN
    RETURN FORMAT('SELECT
        available.id,
        available.id_source AS source,
        available.id_target AS target,
        available.distance_meters AS cost,
        -1 AS reverse_cost, -- unidirectional, prevents insertion
        ST_X(ST_StartPoint(geom)) as x1,
        ST_Y(ST_StartPoint(geom)) as y1,
        ST_X(ST_EndPoint(geom)) as x2,
        ST_Y(ST_EndPoint(geom)) as y2
    FROM
    (
        SELECT ar.id, ar.id_source, ar.id_target, ar.distance_meters, ar.geom
        FROM arrow.available_routes(%L, %L, %L, %L) AS ar
    ) AS available;', $1, $2, $3, $4);
END;
$$ LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = arrow, pg_temp;

CREATE OR REPLACE FUNCTION arrow.best_path (
    start_node_id INTEGER,
    end_node_id INTEGER,
    start_nofly_id INTEGER,
    end_nofly_id INTEGER,
    start_time TIMESTAMPTZ,
    end_time TIMESTAMPTZ
) RETURNS TABLE (
    path_seq INTEGER,
    start_id INTEGER,
    end_id INTEGER,
    distance_meters FLOAT
) AS $body$
BEGIN
    RETURN QUERY
        SELECT
            results.seq,
            ar.id_source,
            ar.id_target,
            results.cost
        FROM arrow.routes ar
        JOIN (
            SELECT seq, edge, cost
            FROM pgr_aStar(
                (SELECT * FROM arrow.a_star_sql_query($3, $4, $5, $6)),
                $1,
                $2,
                directed => true,
                heuristic => 2
            )
        ) AS results ON ar.id = results.edge;
END;
$body$ LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = arrow, public, pg_temp;

CREATE OR REPLACE FUNCTION arrow.best_path_p2p (
    start_node UUID, -- Vertiport
    end_node UUID, -- Vertiport
    start_time TIMESTAMPTZ,
    end_time TIMESTAMPTZ
) RETURNS TABLE (
    path_seq INTEGER,
    start_type arrow.NodeType,
    start_latitude FLOAT,
    start_longitude FLOAT,
    end_type arrow.NodeType,
    end_latitude FLOAT,
    end_longitude FLOAT,
    distance_meters FLOAT
) AS $body$
DECLARE
    start_node_id INTEGER;
    start_nofly_id INTEGER;
    end_node_id INTEGER;
    end_nofly_id INTEGER;
BEGIN
    --- Get Node and Nofly IDs for start and end nodes
    SELECT node_id, nofly_id
        INTO start_node_id, start_nofly_id
        FROM arrow.vertiports WHERE arrow_id = $1;

    SELECT node_id, nofly_id
        INTO end_node_id, end_nofly_id
        FROM arrow.vertiports WHERE arrow_id = $2;

    --- Get best path between nodes

    RETURN QUERY
        SELECT
            results.path_seq,
            (SELECT node_type FROM arrow.nodes WHERE id = results.start_id),
            (SELECT ST_Y(geom) FROM arrow.nodes WHERE id = results.start_id),
            (SELECT ST_X(geom) FROM arrow.nodes WHERE id = results.start_id),
            (SELECT node_type FROM arrow.nodes WHERE id = results.end_id),
            (SELECT ST_Y(geom) FROM arrow.nodes WHERE id = results.end_id),
            (SELECT ST_X(geom) FROM arrow.nodes WHERE id = results.end_id),
            results.distance_meters
        FROM (
            SELECT * FROM arrow.best_path (
                start_node_id,
                end_node_id,
                start_nofly_id,
                end_nofly_id,
                start_time,
                end_time
            )
        ) as results;
END;
$body$ LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = arrow, public, pg_temp;

CREATE OR REPLACE FUNCTION arrow.best_path_a2p (
    start_label VARCHAR, -- Aircraft
    end_node UUID, -- Vertiport
    start_time TIMESTAMPTZ,
    end_time TIMESTAMPTZ
) RETURNS TABLE (
    path_seq INTEGER,
    start_type arrow.NodeType,
    start_latitude FLOAT,
    start_longitude FLOAT,
    end_type arrow.NodeType,
    end_latitude FLOAT,
    end_longitude FLOAT,
    distance_meters FLOAT
) AS $body$
DECLARE
    start_node_id INTEGER;
    end_node_id INTEGER;
    end_nofly_id INTEGER;
BEGIN
    --- Get Node and Nofly IDs for start and end nodes
    SELECT node_id
        INTO start_node_id
        FROM arrow.aircraft WHERE callsign = $1;

    SELECT node_id, nofly_id
        INTO end_node_id, end_nofly_id
        FROM arrow.vertiports WHERE arrow_id = $2;

    -- Build routes for the specific aircraft
    INSERT INTO arrow.routes (
        id_source,
        id_target,
        geom,
        distance_meters
    ) SELECT
        start_node.id,
        end_node.id,
        ST_MakeLine(start_node.geom, end_node.geom),
        ST_DistanceSphere(start_node.geom, end_node.geom)
    FROM arrow.nodes start_node
    INNER JOIN arrow.nodes end_node ON
        (start_node_id = start_node.id) -- Unidirectional routes from aircraft
        AND (end_node.node_type <> 'Aircraft') -- Don't route to aircraft
    ON CONFLICT (id_source, id_target) DO
        UPDATE SET geom = EXCLUDED.geom,
            distance_meters = EXCLUDED.distance_meters;

    --- Get best path between nodes
    RETURN QUERY
        SELECT
            results.path_seq,
            (SELECT node_type FROM arrow.nodes WHERE id = results.start_id),
            (SELECT ST_Y(geom) FROM arrow.nodes WHERE id = results.start_id),
            (SELECT ST_X(geom) FROM arrow.nodes WHERE id = results.start_id),
            (SELECT node_type FROM arrow.nodes WHERE id = results.end_id),
            (SELECT ST_Y(geom) FROM arrow.nodes WHERE id = results.end_id),
            (SELECT ST_X(geom) FROM arrow.nodes WHERE id = results.end_id),
            results.distance_meters
        FROM (
            SELECT * FROM arrow.best_path (
                start_node_id,
                end_node_id,
                NULL,
                end_nofly_id,
                start_time,
                end_time
            )
        ) as results;
END;
$body$ LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = arrow, public, pg_temp;

-- Get the nearest vertiports to a vertiport
CREATE OR REPLACE FUNCTION arrow.nearest_vertiports_to_vertiport (
    start_uuid UUID, -- Vertiport
    n_results INTEGER,
    max_range_meters FLOAT
) RETURNS TABLE (
    arrow_id UUID,
    distance_meters FLOAT
) AS $body$
DECLARE
    start_node_id INTEGER;
BEGIN
    -- Get Node ID of starting vertiport
    SELECT node_id
        FROM arrow.vertiports av
        WHERE av.arrow_id = start_uuid
        INTO start_node_id;

    -- Plug the geometry into a nearest-neighbor query
    RETURN QUERY
        SELECT
            (SELECT av.arrow_id FROM arrow.vertiports av WHERE node_id = ar.id_target),
            ar.distance_meters
            FROM arrow.routes ar
            WHERE
                ar.id_source = start_node_id
                AND ar.distance_meters < max_range_meters
                AND (SELECT node_type FROM arrow.nodes WHERE id = ar.id_target) = 'Vertiport'
            ORDER BY ar.distance_meters
            LIMIT n_results;
END;
$body$ LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = arrow, public, pg_temp;

-- Get the nearest vertiports to an aircraft
CREATE OR REPLACE FUNCTION arrow.nearest_vertiports_to_aircraft (
    start_label VARCHAR, -- Aircraft
    n_results INTEGER,
    max_range_meters FLOAT
) RETURNS TABLE (
    arrow_id UUID,
    distance_meters FLOAT
) AS $body$
DECLARE
    start_node_id INTEGER;
BEGIN
    -- Get Node ID of starting vertiport
    SELECT node_id
        INTO start_node_id
        FROM arrow.aircraft WHERE callsign = start_label;

    -- Plug the geometry into a nearest-neighbor query
    RETURN QUERY
    SELECT
        (SELECT av.arrow_id FROM arrow.vertiports av WHERE node_id = ar.id_target),
        ar.distance_meters
        FROM arrow.routes ar
        WHERE ar.id_source = start_node_id
            AND ar.distance_meters < max_range_meters
            AND (SELECT node_type FROM arrow.nodes WHERE id = ar.id_target) = 'Vertiport'
        ORDER BY ar.distance_meters
        LIMIT n_results;
END;
$body$ LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = arrow, public, pg_temp;

-- These permissions must be declared last
GRANT USAGE ON SCHEMA arrow TO svc_gis;
GRANT EXECUTE ON FUNCTION arrow.update_aircraft_position(
    UUID,
    GEOMETRY(Point),
    FLOAT,
    VARCHAR,
    TIMESTAMPTZ
) TO svc_gis;

GRANT EXECUTE ON FUNCTION arrow.update_nofly(
    VARCHAR,
    GEOMETRY(Polygon),
    TIMESTAMPTZ,
    TIMESTAMPTZ
) TO svc_gis;

GRANT EXECUTE ON FUNCTION arrow.update_vertiport(
    UUID,
    GEOMETRY(Polygon),
    VARCHAR
) TO svc_gis;

GRANT EXECUTE ON FUNCTION arrow.update_waypoint(
    VARCHAR,
    GEOMETRY(Point)
) TO svc_gis;

GRANT EXECUTE ON FUNCTION arrow.best_path_p2p(
    UUID,
    UUID,
    TIMESTAMPTZ,
    TIMESTAMPTZ
) TO svc_gis;

GRANT EXECUTE ON FUNCTION arrow.best_path_a2p(
    VARCHAR,
    UUID,
    TIMESTAMPTZ,
    TIMESTAMPTZ
) TO svc_gis;

GRANT EXECUTE ON FUNCTION arrow.nearest_vertiports_to_vertiport(
    UUID,
    INTEGER,
    FLOAT
) TO svc_gis;

GRANT EXECUTE ON FUNCTION arrow.nearest_vertiports_to_aircraft(
    VARCHAR,
    INTEGER,
    FLOAT
) TO svc_gis;
