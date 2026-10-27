SELECT /* lemmesee:discovery:pg.n.tables */ c.oid AS table_oid, c.reltuples, c.relpages,
       s.n_live_tup, s.n_dead_tup, s.n_tup_ins, s.n_tup_upd, s.n_tup_hot_upd, s.n_tup_del,
       s.n_mod_since_analyze, s.last_analyze, s.last_autoanalyze,
       s.analyze_count, s.autoanalyze_count
FROM pg_class c
JOIN pg_namespace n ON n.oid = c.relnamespace
LEFT JOIN pg_stat_all_tables s ON s.relid = c.oid
WHERE c.relkind IN ('r', 'p', 'm')
  AND n.nspname NOT IN ('pg_catalog', 'information_schema')
  AND n.nspname NOT LIKE 'pg\_toast%' AND n.nspname NOT LIKE 'pg\_temp%'
  -- objects an extension owns, such as pg_stat_statements' views, are not the application's
  AND NOT EXISTS (SELECT FROM pg_depend d WHERE d.classid = 'pg_class'::regclass AND d.objid = c.oid AND d.deptype = 'e')
ORDER BY c.oid
