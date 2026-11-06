# what one read sends on MariaDb, from the server's general log

(b), 8 commands:
    Connect      reader@172.28.0.1 on probe using TCP/IP
    Query        SELECT @@max_allowed_packet,@@wait_timeout
    Query        SET SESSION transaction_read_only = ON, time_zone = '+00:00', max_statement_time = 5
    Query        START TRANSACTION READ ONLY
    Prepare      SELECT COUNT(*) FROM probe.big WHERE created_at >= CAST(? AS DATETIME(6)) AND created_at < CAST(? AS DATETIME(
    Execute      SELECT COUNT(*) FROM probe.big WHERE created_at >= CAST('2026-10-02 00:00:00' AS DATETIME(6)) AND created_at <
    Query        ROLLBACK
    Quit         
(a), 9 commands:
    Connect      reader@172.28.0.1 on probe using TCP/IP
    Query        SELECT @@max_allowed_packet,@@wait_timeout
    Query        SET SESSION transaction_read_only = ON, time_zone = '+00:00', max_statement_time = 5
    Query        START TRANSACTION READ ONLY
    Prepare      SET @from = CAST(? AS DATETIME(6)), @to = CAST(? AS DATETIME(6))
    Execute      SET @from = CAST('2026-10-02 00:00:00' AS DATETIME(6)), @to = CAST('2026-10-03 00:00:00' AS DATETIME(6))
    Query        SELECT COUNT(*) FROM probe.big WHERE created_at >= @from AND created_at < @to
    Query        ROLLBACK
    Quit         
