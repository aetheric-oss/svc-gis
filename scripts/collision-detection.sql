SELECT geom FROM arrow.rnodes
UNION
SELECT geom FROM arrow.routes
UNION
SELECT geom FROM arrow.nofly
UNION
SELECT position FROM arrow.aircraft
UNION
SELECT aircraft.flashlight
FROM (
    SELECT a1.flashlight FROM arrow.aircraft a1
    INNER JOIN arrow.aircraft a2
    ON a1.icao <> a2.icao AND ST_Intersects(a1.flashlight, a2.flashlight)
) as aircraft;
