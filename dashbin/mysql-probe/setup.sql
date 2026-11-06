-- the probe's data, the same on MySQL 8.4 and MariaDB 11; run as root
SET GLOBAL log_bin_trust_function_creators = 1;
DROP DATABASE IF EXISTS probe;
CREATE DATABASE probe;
USE probe;

-- one row of edges for each type a frame might read, and a row of nulls
CREATE TABLE types (
    id INT PRIMARY KEY,
    d65 DECIMAL(65, 30),
    d12 DECIMAL(12, 2),
    u64 BIGINT UNSIGNED,
    i64 BIGINT,
    i8 TINYINT,
    flag TINYINT(1),
    f32 FLOAT,
    f64 DOUBLE,
    day DATE,
    wall0 DATETIME,
    wall3 DATETIME(3),
    wall6 DATETIME(6),
    instant6 TIMESTAMP(6) NULL,
    clock TIME(6),
    yr YEAR,
    word VARCHAR(40) CHARACTER SET utf8mb4,
    latin VARCHAR(40) CHARACTER SET latin1,
    bytes VARBINARY(8),
    bits BIT(10),
    doc JSON,
    state ENUM('open', 'paid'),
    tags SET('a', 'b', 'c'),
    spot POINT
);
INSERT INTO types VALUES
    (1, '99999999999999999999999999999999999.999999999999999999999999999999', 9999999999.99, 18446744073709551615,
     -9223372036854775808, -128, 1, 0.1, 0.1, '2026-11-01', '9999-12-31 23:59:59', '2026-11-01 00:00:00.120',
     '2026-11-01 00:00:00.000001', '2026-11-01 00:00:00.500000', '-838:59:59.000000', 2155, 'кот 🐈 "q" \\ ''',
     'café', 0x00FF10, b'1010101010', '{"b": 1, "a": [1.5, null]}', 'paid', 'a,c', ST_GeomFromText('POINT(1 2)')),
    (2, '-0.000000000000000000000000000001', -0.01, 0, 9223372036854775807, 127, 0, -3.4e38, 1e308,
     '1000-01-01', '1000-01-01 00:00:00', '1000-01-01 00:00:00.000', '1970-01-01 00:00:00.000000',
     '1970-01-01 00:00:01.000000', '838:59:59.000000', 1901, '', '', '', b'0', 'null', 'open', '', NULL),
    (3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
     NULL, NULL, NULL, NULL, NULL);

-- 168 000 orders, as the PostgreSQL probe's
CREATE TABLE big (
    id INT PRIMARY KEY AUTO_INCREMENT,
    created_at TIMESTAMP(6) NOT NULL,
    stamped DATETIME(6) NOT NULL,
    amount DECIMAL(12, 2) NOT NULL,
    note VARCHAR(64) NOT NULL,
    KEY (created_at)
);
CREATE TABLE digits (d INT);
INSERT INTO digits VALUES (0), (1), (2), (3), (4), (5), (6), (7), (8), (9);
INSERT INTO big (created_at, stamped, amount, note)
SELECT TIMESTAMP('2026-10-01') + INTERVAL i * 15 SECOND, TIMESTAMP('2026-10-01') + INTERVAL i * 15 SECOND,
       (i * 37 % 100000) / 100, MD5(i)
FROM (SELECT a.d + 10 * b.d + 100 * c.d + 1000 * e.d + 10000 * f.d + 100000 * g.d + 1 AS i
      FROM digits a, digits b, digits c, digits e, digits f, digits g) n
WHERE i <= 168000;

-- what an attack must not change
CREATE TABLE victim (x INT);
INSERT INTO victim VALUES (1);

-- a function that writes, and a procedure that tries to leave a read-only transaction and write
DELIMITER //
CREATE FUNCTION writes() RETURNS INT MODIFIES SQL DATA SQL SECURITY DEFINER
BEGIN
    INSERT INTO victim VALUES (2);
    RETURN 1;
END //
CREATE PROCEDURE escapes() SQL SECURITY DEFINER
BEGIN
    COMMIT;
    SET SESSION transaction_read_only = OFF;
    START TRANSACTION READ WRITE;
    INSERT INTO victim VALUES (3);
    COMMIT;
    SELECT 'escaped' AS said;
END //
DELIMITER ;

-- a reader as an operator should make one, and a writer with every right on the database, to see what the layers
-- hold without the operator's help
DROP USER IF EXISTS reader, writer;
CREATE USER reader IDENTIFIED BY 'probe-reader';
GRANT SELECT ON probe.* TO reader;
CREATE USER writer IDENTIFIED BY 'probe-writer';
GRANT ALL ON probe.* TO writer;
