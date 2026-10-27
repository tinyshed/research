SELECT /* lemmesee:discovery:pg.s.columns */ c.oid AS table_oid, a.attnum, a.attname,
       format_type(a.atttypid, a.atttypmod) AS type_name,
       format_type(COALESCE(NULLIF(t.typbasetype, 0), a.atttypid), NULL) AS base_type,
       t.typtype, NOT a.attnotnull AS nullable,
       NULLIF(a.attidentity, '') AS identity, NULLIF(a.attgenerated, '') AS generated,
       pg_get_expr(d.adbin, d.adrelid) AS default_expr,
       col_description(c.oid, a.attnum) AS comment,
       has_column_privilege(c.oid, a.attnum, 'SELECT') AS can_select,
       (SELECT string_agg(e.enumlabel, E'\t' ORDER BY e.enumsortorder)
          FROM pg_enum e WHERE e.enumtypid = a.atttypid) AS enum_labels
FROM pg_attribute a
JOIN pg_class c ON c.oid = a.attrelid
JOIN pg_namespace n ON n.oid = c.relnamespace
JOIN pg_type t ON t.oid = a.atttypid
LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
WHERE a.attnum > 0 AND NOT a.attisdropped
  AND c.relkind IN ('r', 'p', 'v', 'm', 'f')
  AND n.nspname NOT IN ('pg_catalog', 'information_schema')
  AND n.nspname NOT LIKE 'pg\_toast%' AND n.nspname NOT LIKE 'pg\_temp%'
  -- objects an extension owns, such as pg_stat_statements' views, are not the application's
  AND NOT EXISTS (SELECT FROM pg_depend d WHERE d.classid = 'pg_class'::regclass AND d.objid = c.oid AND d.deptype = 'e')
ORDER BY c.oid, a.attnum
