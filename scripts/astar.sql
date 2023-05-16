SELECT geom FROM arrow.nofly
UNION
SELECT geom FROM arrow.rnodes
UNION
SELECT geom
FROM arrow.routes ar
JOIN (
    SELECT path_seq, edge, cost
    FROM pgr_aStar(
        'SELECT
            results.id,
            id_source AS source,
            id_target AS target,
            distance AS cost,
            distance AS reverse_cost,
            ST_X(ST_StartPoint(geom)) as x1,
            ST_Y(ST_StartPoint(geom)) as y1,
            ST_X(ST_EndPoint(geom)) as x2,
            ST_Y(ST_EndPoint(geom)) as y2
        FROM
        (
            SELECT ar.id, id_source, id_target, distance, ar.geom
            FROM arrow.routes AS ar
            LEFT JOIN arrow.nofly AS anf
                ON ST_Intersects(ar.geom, anf.geom)
                WHERE (anf.time_start IS NULL AND anf.time_end IS NULL)
                AND anf.id IS NULL
        ) AS results',
        (SELECT arn.id FROM arrow.rnodes AS arn WHERE arn.arrow_id = '00000000-0000-0000-0000-000000000001'),
        (SELECT arn.id FROM arrow.rnodes AS arn WHERE arn.arrow_id = '00000000-0000-0000-0000-000000000002'),
        directed => true,
        heuristic => 2
    )
) AS results ON ar.id = results.edge;
