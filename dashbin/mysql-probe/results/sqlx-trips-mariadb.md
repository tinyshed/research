# what one read sends through sqlx on 13307, 9 commands (one asks its id)

    Connect      reader@172.28.0.1 on probe using TCP/IP
    Prepare      SELECT CONNECTION_ID()
    Execute      SELECT CONNECTION_ID()
    Query        SET SESSION transaction_read_only = ON, time_zone = '+00:00'
    Query        START TRANSACTION READ ONLY
    Prepare      SELECT COUNT(*) FROM probe.big WHERE created_at >= CAST(? AS DATETIME(6)) AND created_at < CAST(? AS
    Execute      SELECT COUNT(*) FROM probe.big WHERE created_at >= CAST('2026-10-02 00:00:00' AS DATETIME(6)) AND cr
    Query        ROLLBACK
    Quit         
