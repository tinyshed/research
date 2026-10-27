SELECT /* lemmesee:discovery:pg.q.workload */ EXISTS (SELECT FROM pg_extension WHERE extname = 'pg_stat_statements') AS installed,
       pg_has_role('pg_read_all_stats', 'MEMBER') AS reads_all
