SELECT /* lemmesee:discovery:pg.s.tables */ c.oid, n.nspname AS schema, c.relname AS name, c.relkind AS kind,
       (SELECT pn.nspname || '.' || pc.relname
          FROM pg_inherits i
          JOIN pg_class pc ON pc.oid = i.inhparent
          JOIN pg_namespace pn ON pn.oid = pc.relnamespace
         WHERE i.inhrelid = c.oid AND c.relispartition) AS partition_of,
       obj_description(c.oid, 'pg_class') AS comment,
       has_table_privilege(c.oid, 'SELECT') AS can_select,
       row_security_active(c.oid) AS row_security_active
FROM pg_class c
JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE c.relkind IN ('r', 'p', 'v', 'm', 'f')
  AND n.nspname NOT IN ('pg_catalog', 'information_schema')
  AND n.nspname NOT LIKE 'pg\_toast%' AND n.nspname NOT LIKE 'pg\_temp%'
  -- objects an extension owns, such as pg_stat_statements' views, are not the application's
  AND NOT EXISTS (SELECT FROM pg_depend d WHERE d.classid = 'pg_class'::regclass AND d.objid = c.oid AND d.deptype = 'e')
ORDER BY c.oid
