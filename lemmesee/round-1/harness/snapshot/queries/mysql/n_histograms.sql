SELECT /* lemmesee:discovery:my.n.histograms */ cs.TABLE_NAME, cs.COLUMN_NAME,
       ANY_VALUE(cs.HISTOGRAM->>'$."histogram-type"')    AS kind,
       ANY_VALUE(cs.HISTOGRAM->>'$."null-values"')       AS null_frac,
       ANY_VALUE(cs.HISTOGRAM->>'$."sampling-rate"')     AS sampling_rate,
       ANY_VALUE(cs.HISTOGRAM->>'$."last-updated"')      AS last_updated,
       ANY_VALUE(JSON_LENGTH(cs.HISTOGRAM->'$.buckets')) AS buckets,
       SUM(jt.nd)                                        AS distinct_sum
FROM information_schema.COLUMN_STATISTICS cs
-- JSON_TABLE takes a column of a table before it in the same FROM, and refuses
-- COLUMN_STATISTICS.HISTOGRAM unless it is cast to JSON
LEFT JOIN JSON_TABLE(CAST(cs.HISTOGRAM AS JSON), '$.buckets[*]' COLUMNS (nd BIGINT PATH '$[3]')) jt ON TRUE
WHERE cs.SCHEMA_NAME = DATABASE()
GROUP BY cs.TABLE_NAME, cs.COLUMN_NAME
