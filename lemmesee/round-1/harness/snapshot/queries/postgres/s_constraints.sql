SELECT /* lemmesee:discovery:pg.s.constraints */ con.conrelid AS table_oid, con.conname AS name, con.contype AS kind,
       (SELECT string_agg(a.attname, E'\t' ORDER BY k.ord)
          FROM unnest(con.conkey) WITH ORDINALITY k(attnum, ord)
          JOIN pg_attribute a ON a.attrelid = con.conrelid AND a.attnum = k.attnum) AS columns,
       (SELECT rn.nspname || '.' || rc.relname
          FROM pg_class rc JOIN pg_namespace rn ON rn.oid = rc.relnamespace
         WHERE rc.oid = con.confrelid) AS ref_table,
       (SELECT string_agg(a.attname, E'\t' ORDER BY k.ord)
          FROM unnest(con.confkey) WITH ORDINALITY k(attnum, ord)
          JOIN pg_attribute a ON a.attrelid = con.confrelid AND a.attnum = k.attnum) AS ref_columns,
       pg_get_constraintdef(con.oid) AS definition
FROM pg_constraint con
JOIN pg_class c ON c.oid = con.conrelid
JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE con.contype IN ('p', 'u', 'f', 'c')
  AND n.nspname NOT IN ('pg_catalog', 'information_schema')
  AND n.nspname NOT LIKE 'pg\_toast%' AND n.nspname NOT LIKE 'pg\_temp%'
  -- objects an extension owns, such as pg_stat_statements' views, are not the application's
  AND NOT EXISTS (SELECT FROM pg_depend d WHERE d.classid = 'pg_class'::regclass AND d.objid = c.oid AND d.deptype = 'e')
ORDER BY con.conrelid, con.conname
