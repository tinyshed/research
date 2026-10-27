SELECT /* lemmesee:discovery:pg.n.columns */ s.schemaname, s.tablename, s.attname, s.inherited,
       s.null_frac, s.avg_width, s.n_distinct, s.correlation,
       array_length(s.most_common_freqs, 1)               AS mcv_count,
       s.most_common_freqs[1]                             AS mcv_top_freq,
       (SELECT sum(f) FROM unnest(s.most_common_freqs) f) AS mcv_freq_sum,
       array_length(s.histogram_bounds::text::text[], 1)  AS hist_bounds
FROM pg_stats s
WHERE s.schemaname NOT IN ('pg_catalog', 'information_schema')
