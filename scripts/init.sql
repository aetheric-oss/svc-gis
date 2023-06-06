CREATE DATABASE gis;
CREATE USER svc_gis;
\c gis

CREATE SCHEMA arrow;
CREATE EXTENSION postgis;
CREATE EXTENSION pgrouting CASCADE;

CREATE TYPE NodeType AS ENUM ('waypoint', 'vertiport');

-- Vertiports and waypoints
CREATE TABLE arrow.rnodes (
    id SERIAL PRIMARY KEY NOT NULL,
    arrow_id uuid UNIQUE NOT NULL,
    node_type NodeType NOT NULL,
    geom GEOMETRY(Point) NOT NULL
);

CREATE TABLE arrow.routes (
    id SERIAL NOT NULL,
    id_source INTEGER NOT NULL,
    id_target INTEGER NOT NULL,
    geom GEOMETRY(LineString) NOT NULL,
    distance_meters float NOT NULL,
    PRIMARY KEY (id_source, id_target),
    CONSTRAINT fk_source
        FOREIGN KEY (id_source)
        REFERENCES arrow.rnodes(id),
    CONSTRAINT fk_target
        FOREIGN KEY (id_target)
        REFERENCES arrow.rnodes(id)
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

--
-- TRIGGERS START
--

-- When a node is updated, the associated routes must be updated
--  as well. This trigger will update the routes table with the
--  new geometry and distance.
CREATE FUNCTION route_update()
    RETURNS TRIGGER
AS $$
BEGIN
    INSERT INTO arrow.routes (
        id_source,
        id_target,
        geom,
        distance_meters
    )
    SELECT
        ar.id,
        o.id,
        ST_MakeLine(ar.geom, o.geom),
        ST_DistanceSphere(ar.geom, o.geom)
    FROM arrow.rnodes ar
    INNER JOIN arrow.rnodes o ON
        (NEW.arrow_id IN (ar.arrow_id, o.arrow_id))
        AND (ar.id <> o.id) -- Build two unidirectional routes
    ON CONFLICT (id_source, id_target)
        DO UPDATE
            SET geom = EXCLUDED.geom,
                distance_meters = EXCLUDED.distance_meters;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER route_update
    AFTER UPDATE OR INSERT
    ON arrow.rnodes
    FOR EACH ROW
    EXECUTE PROCEDURE route_update();

-- When a node is deleted, the associated routes must be deleted first,
--  as the routes table has foreign key constraints on the nodes table.
CREATE FUNCTION route_delete()
    RETURNS TRIGGER
AS $$
BEGIN
    DELETE FROM arrow.routes ar
    WHERE
        (OLD.id IN (ar.id_source, ar.id_target));

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER route_delete
    BEFORE DELETE
    ON arrow.rnodes
    FOR EACH ROW
    EXECUTE PROCEDURE route_delete();

--
-- END TRIGGERS
--

--
-- Routing Algorithms
--
CREATE OR REPLACE FUNCTION available_routes (timestamptz, timestamptz)
RETURNS TABLE (id integer, id_source integer, id_target integer, distance_meters double precision, geom geometry)
AS $$
BEGIN
    RETURN QUERY
        SELECT ar.id, ar.id_source, ar.id_target, ar.distance_meters, ar.geom
        FROM arrow.routes AS ar
        LEFT JOIN
        (
            SELECT anf.id, anf.geom FROM arrow.nofly AS anf
            WHERE (
                (anf.time_start IS NULL AND anf.time_end IS NULL) -- No-Fly Zone is permanent
                OR (
                    (anf.time_start < $2)
                    AND (anf.time_end > $1)
                ) -- Temporary Flight Restriction
            )
        ) AS anf
        ON ST_Intersects(ar.geom, anf.geom)
        WHERE anf.id IS NULL;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION a_star_sql_query (timestamptz, timestamptz)
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
        FROM available_routes(%L, %L) AS ar
    ) AS available;', $1, $2);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION best_path (uuid, uuid, timestamptz, timestamptz)
RETURNS TABLE (seq integer, start_id uuid, end_id uuid, distance_meters double precision)
AS $body$
BEGIN
    RETURN QUERY
        SELECT
            results.path_seq,
            (SELECT arrow_id FROM arrow.rnodes arn WHERE arn.id = ar.id_source),
            (SELECT arrow_id FROM arrow.rnodes arn WHERE arn.id = ar.id_target),
            ar.distance_meters
        FROM arrow.routes ar
        JOIN (
            SELECT path_seq, edge, cost
            FROM pgr_aStar(
                (SELECT * FROM a_star_sql_query($3, $4)),
                (SELECT arn.id FROM arrow.rnodes AS arn WHERE arn.arrow_id = $1),
                (SELECT arn.id FROM arrow.rnodes AS arn WHERE arn.arrow_id = $2),
                directed => true,
                heuristic => 2
            )
        ) AS results ON ar.id = results.edge;
END;
$body$ LANGUAGE plpgsql;

-- These permissions must be declared last
GRANT ALL ON SCHEMA arrow TO svc_gis;
GRANT ALL
    ON ALL TABLES IN SCHEMA arrow
    TO svc_gis;
GRANT ALL
    ON ALL SEQUENCES IN SCHEMA arrow
    TO svc_gis;
