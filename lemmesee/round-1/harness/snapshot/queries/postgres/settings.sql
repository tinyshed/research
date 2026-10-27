SELECT /* lemmesee:discovery:pg.settings */ current_setting('server_version') AS server_version,
       current_database() AS database, current_user AS role, now()::text AS taken_at,
       current_setting('default_statistics_target') AS default_statistics_target,
       current_setting('autovacuum') AS autovacuum,
       current_setting('track_counts') AS track_counts
