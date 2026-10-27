SELECT /* lemmesee:discovery:my.settings */ @@version AS server_version, DATABASE() AS `database`, CURRENT_USER() AS role,
       CAST(NOW(6) AS CHAR) AS taken_at,
       @@information_schema_stats_expiry AS information_schema_stats_expiry,
       @@innodb_stats_persistent AS innodb_stats_persistent,
       @@innodb_stats_auto_recalc AS innodb_stats_auto_recalc,
       @@innodb_stats_persistent_sample_pages AS innodb_stats_persistent_sample_pages
