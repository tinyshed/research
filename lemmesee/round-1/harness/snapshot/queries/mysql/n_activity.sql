SELECT /* lemmesee:discovery:my.n.activity */ OBJECT_NAME AS TABLE_NAME, COUNT_INSERT, COUNT_UPDATE, COUNT_DELETE
FROM performance_schema.table_io_waits_summary_by_table
WHERE OBJECT_SCHEMA = DATABASE()
