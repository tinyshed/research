SELECT substring(query FROM 'lemmesee:([a-z]+:[a-z0-9._]+)') AS id,
       calls, total_exec_time, rows, shared_blks_hit, shared_blks_read
FROM pg_stat_statements
WHERE query LIKE '%/* lemmesee:%'
