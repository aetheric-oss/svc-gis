CREATE DATABASE gis;
CREATE USER svc_gis;
\c gis

CREATE SCHEMA arrow;
REVOKE ALL ON SCHEMA public FROM PUBLIC;
CREATE EXTENSION postgis;
CREATE EXTENSION pgrouting CASCADE;
SET search_path TO "$user", postgis, topology, public;
ALTER ROLE svc_gis SET search_path TO "$user", postgis, topology, public;

--
-- Nodes for Path Finding
--
CREATE TYPE arrow.NodeType AS ENUM ('waypoint', 'vertiport', 'aircraft');
CREATE TYPE arrow.NoFlyType AS ENUM ('nofly', 'vertiport');

CREATE TABLE arrow.nodes (
    id SERIAL PRIMARY KEY NOT NULL,
    node_type arrow.NodeType NOT NULL
);

CREATE TABLE arrow.node_locations (
    node_id INTEGER NOT NULL,
    time_point TIMESTAMPTZ NOT NULL,
    geom GEOMETRY(Point) NOT NULL,
    PRIMARY KEY(node_id, time_point),
    CONSTRAINT fk_node_id
        FOREIGN KEY (node_id)
        REFERENCES arrow.nodes(id)
);

CREATE INDEX nodes_idx
    ON arrow.node_locations
    USING GIST (geom);

---
--- Edges in Path Finding
---
-- CREATE TABLE arrow.routes (
--     id SERIAL NOT NULL,
--     id_source INTEGER NOT NULL,
--     id_target INTEGER NOT NULL,
--     geom GEOMETRY(LineString) NOT NULL,
--     distance_meters FLOAT NOT NULL,
--     PRIMARY KEY (id_source, id_target)
--     CONSTRAINT fk_source
--         FOREIGN KEY (id_source)
--         REFERENCES arrow.nodes(id),
--     CONSTRAINT fk_target
--         FOREIGN KEY (id_target)
--         REFERENCES arrow.nodes(id)
-- );

-- CREATE INDEX routes_idx
--     ON arrow.routes
--     USING GIST (geom);

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
        SELECT vs.node_id, vs.nofly_id
        INTO node_id, nofly_id
        FROM arrow.vertiports vs
        WHERE vs.arrow_id = port_uuid;

        INSERT INTO arrow.node_locations (
            node_id,
            time_point,
            geom
        ) VALUES (
            node_id,
            NOW(),
            arrow.centroid(port_geom)
        );

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

    INSERT INTO arrow.nodes (node_type)
        VALUES ('vertiport')
        RETURNING id INTO node_id;

    INSERT INTO arrow.node_locations (node_id, geom, time_point)
        VALUES (node_id, arrow.centroid(port_geom), NOW());

    INSERT INTO arrow.nofly (label, geom, nofly_type, time_start, time_end)
        VALUES (port_label, port_geom, 'vertiport', NULL, NULL)
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

        INSERT INTO arrow.node_locations (
            node_id,
            time_point,
            geom
        ) VALUES (
            node_id,
            NOW(),
            pt_geom
        );

        RETURN;
    END IF;

    -- Vertiports are both nodes and nofly zones
    -- The Nodes and No-Fly Zones should be created first
    INSERT INTO arrow.nodes (node_type)
        VALUES ('waypoint')
        RETURNING id INTO node_id;

    INSERT INTO arrow.node_locations (node_id, geom, time_point)
        VALUES (node_id, pt_geom, NOW());

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
        VALUES (nofly_label, nofly_geom, 'nofly', nofly_time_start, nofly_time_end)
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
RETURNS boolean AS $$
DECLARE
    node_id INTEGER;
BEGIN
    IF craft_callsign IS NULL THEN
        RETURN FALSE;
    END IF;

    IF craft_callsign IN (SELECT callsign FROM arrow.aircraft) THEN
        -- Don't overwrite with older information
        IF craft_time < (SELECT last_updated FROM arrow.aircraft WHERE callsign = craft_callsign) THEN
            RETURN FALSE;
        END IF;

        SELECT air.node_id INTO node_id
            FROM arrow.aircraft air WHERE callsign = craft_callsign;

        -- Overwrite old estimates
        DELETE FROM arrow.node_locations
            WHERE node_id = node_id
                AND time_point > craft_time;

        INSERT INTO arrow.node_locations (
            node_id,
            time_point,
            geom
        ) VALUES (
            node_id,
            craft_time,
            craft_geom
        );

        UPDATE arrow.aircraft
            SET altitude_meters = craft_altitude_m,
                last_updated = craft_time,
            WHERE callsign = craft_callsign;

        IF craft_uuid IS NOT NULL THEN
            UPDATE arrow.aircraft
                SET arrow_id = craft_uuid
                WHERE callsign = craft_callsign;
        END IF;

        RETURN TRUE;
    END IF;

    -- Vertiports are both nodes and nofly zones
    -- The Nodes and No-Fly Zones should be created first

    INSERT INTO arrow.nodes (node_type)
        VALUES ('aircraft')
        RETURNING id INTO node_id;

    INSERT INTO arrow.node_locations (node_id, geom, time_point)
        VALUES (node_id, craft_geom, craft_time);

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

    RETURN TRUE;
END; $$ LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = arrow, pg_temp;

CREATE OR REPLACE FUNCTION arrow.get_nodes_at(
    target_time TIMESTAMPTZ,
    tolerance INTERVAL
)
RETURNS TABLE (
    node_id INTEGER,
    time_point TIMESTAMPTZ,
    geom GEOMETRY(Point),
    diff INTERVAL
)
AS $$
DECLARE
    current_time TIMESTAMPTZ;
BEGIN
    RETURN QUERY
        SELECT DISTINCT ON (an.node_id)
            an.node_id,
            an.time_point,
            an.geom,
            ABS(target_time - an.time_point) as diff
        FROM arrow.node_locations an
        WHERE
            ( ABS(target_time - an.time_point) < tolerance )
            OR ((SELECT node_type FROM arrow.nodes WHERE id = an.node_id) <> 'aircraft')
        ORDER BY an.node_id ASC, ABS(target_time - an.time_point) ASC;
END;
$$ LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = arrow, public, pg_temp;

CREATE OR REPLACE FUNCTION abs(interval) RETURNS interval AS
  $$ select case when ($1<interval '0') then -$1 else $1 end; $$
LANGUAGE sql immutable;

CREATE OR REPLACE FUNCTION arrow.get_routes(
--     start_node_id INTEGER, -- corners of area to search
--     end_node_id INTEGER,
    time_point TIMESTAMPTZ,
    tolerance INTERVAL
)
RETURNS TABLE (
    id INTEGER,
    id_source INTEGER,
    id_target INTEGER,
    geom GEOMETRY(LineString),
    distance_meters FLOAT
)
AS $$
DECLARE
    distance FLOAT;
BEGIN
    --- Get Node Locations At This Time
    RETURN QUERY
        WITH points AS (
            SELECT * FROM arrow.get_nodes_at(time_point, tolerance) n
        )
        SELECT
            CAST(row_number() OVER () AS INTEGER) AS id,
            start_node.node_id,
            end_node.node_id,
            ST_MakeLine(start_node.geom, end_node.geom),
            ST_DistanceSphere(start_node.geom, end_node.geom)
        FROM points start_node
        INNER JOIN points end_node
        ON (start_node.node_id <> end_node.node_id) -- Build two unidirectional routes
            AND ((SELECT node_type FROM arrow.nodes an WHERE an.id = end_node.node_id) <> 'aircraft') -- Don't route to aircraft
        ORDER BY id;
END;
$$ LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = arrow, public, pg_temp;

--------------------------------------------------------------------------------
-- TRIGGERS
--------------------------------------------------------------------------------
-- \i triggers.sql

-- Must delete related nodes and nofly zones when a vertiport is deleted
CREATE OR REPLACE FUNCTION arrow.vertiport_cleanup() RETURNS trigger AS $$
    BEGIN
        DELETE FROM arrow.node_locations
            WHERE (OLD.node_id = node_id);
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
        DELETE FROM arrow.node_locations
            WHERE (OLD.node_id = node_id);

        DELETE FROM arrow.nodes an
            WHERE an.id = OLD.node_id;
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
)
STABLE
AS $$
BEGIN
    RETURN QUERY
        SELECT ar.id, ar.id_source, ar.id_target, ar.distance_meters, ar.geom
        FROM (
            SELECT * FROM arrow.get_routes(time_start, INTERVAL '1 hour')
        ) AS ar
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
        routes.id_source,
        routes.id_target,
        results.cost
    FROM arrow.available_routes(
        start_nofly_id, end_nofly_id, start_time, end_time
    ) as routes
    JOIN (
        SELECT seq, edge, cost
        FROM pgr_aStar(
            FORMAT('SELECT
                ar.id,
                ar.id_source AS source,
                ar.id_target AS target,
                ar.distance_meters AS cost,
                -1 AS reverse_cost, -- unidirectional, prevents insertion
                ST_X(ST_StartPoint(ar.geom)) as x1,
                ST_Y(ST_StartPoint(ar.geom)) as y1,
                ST_X(ST_EndPoint(ar.geom)) as x2,
                ST_Y(ST_EndPoint(ar.geom)) as y2
            FROM arrow.available_routes(%L, %L, %L, %L) as ar',
            start_nofly_id, end_nofly_id, start_time, end_time),
            $1,
            $2,
            directed => true,
            heuristic => 2
        )
    ) AS results ON routes.id = results.edge;
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

    RETURN QUERY
        SELECT
            results.path_seq,
            (SELECT node_type FROM arrow.nodes WHERE id = results.start_id),
            (SELECT ST_Y(geom) FROM arrow.node_locations WHERE node_id = results.start_id),
            (SELECT ST_X(geom) FROM arrow.node_locations WHERE node_id = results.start_id),
            (SELECT node_type FROM arrow.nodes WHERE id = results.end_id),
            (SELECT ST_Y(geom) FROM arrow.node_locations WHERE node_id = results.end_id),
            (SELECT ST_X(geom) FROM arrow.node_locations WHERE node_id = results.end_id),
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

    --- Get best path between nodes
    RETURN QUERY
        SELECT
            results.path_seq,
            (SELECT node_type FROM arrow.nodes WHERE id = results.start_id),
            (SELECT ST_Y(geom) FROM arrow.node_locations WHERE node_id = results.start_id),
            (SELECT ST_X(geom) FROM arrow.node_locations WHERE node_id = results.start_id),
            (SELECT node_type FROM arrow.nodes WHERE id = results.end_id),
            (SELECT ST_Y(geom) FROM arrow.node_locations WHERE node_id = results.end_id),
            (SELECT ST_X(geom) FROM arrow.node_locations WHERE node_id = results.end_id),
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
