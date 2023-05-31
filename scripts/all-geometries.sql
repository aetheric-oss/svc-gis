SELECT geom FROM arrow.rnodes
UNION
SELECT geom FROM arrow.routes
UNION
SELECT geom FROM arrow.nofly;
