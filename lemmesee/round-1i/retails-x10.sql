-- Round 1i: bird_retails with ten times its orders and their lines, keys moved past the last, for timing at scale;
-- run on a copy: CREATE DATABASE bird_retails_x10 TEMPLATE bird_retails, then psql -d bird_retails_x10 < retails-x10.sql
\set ON_ERROR_STOP 1
\timing on
ALTER TABLE lineitem DROP CONSTRAINT lineitem_l_orderkey_fkey, DROP CONSTRAINT lineitem_l_partkey_l_suppkey_fkey;
ALTER TABLE orders DROP CONSTRAINT orders_o_custkey_fkey;
SELECT max(o_orderkey) AS m FROM orders \gset
INSERT INTO orders (o_orderkey, o_custkey, o_orderstatus, o_totalprice, o_orderdate, o_orderpriority, o_clerk, o_shippriority, o_comment) SELECT o_orderkey + g * :m, o_custkey, o_orderstatus, o_totalprice, o_orderdate, o_orderpriority, o_clerk, o_shippriority, o_comment
  FROM orders, generate_series(1, 9) g;
INSERT INTO lineitem (l_orderkey, l_partkey, l_suppkey, l_linenumber, l_quantity, l_extendedprice, l_discount, l_tax, l_returnflag, l_linestatus, l_shipdate, l_commitdate, l_receiptdate, l_shipinstruct, l_shipmode, l_comment) SELECT l_orderkey + g * :m, l_partkey, l_suppkey, l_linenumber, l_quantity, l_extendedprice, l_discount, l_tax, l_returnflag, l_linestatus, l_shipdate, l_commitdate, l_receiptdate, l_shipinstruct, l_shipmode, l_comment
  FROM lineitem, generate_series(1, 9) g;
ALTER TABLE lineitem ADD CONSTRAINT lineitem_l_orderkey_fkey FOREIGN KEY (l_orderkey) REFERENCES orders(o_orderkey) NOT VALID;
ALTER TABLE lineitem ADD CONSTRAINT lineitem_l_partkey_l_suppkey_fkey FOREIGN KEY (l_partkey, l_suppkey) REFERENCES partsupp(ps_partkey, ps_suppkey) NOT VALID;
ALTER TABLE orders ADD CONSTRAINT orders_o_custkey_fkey FOREIGN KEY (o_custkey) REFERENCES customer(c_custkey) NOT VALID;
VACUUM ANALYZE orders;
VACUUM ANALYZE lineitem;
SELECT relname, reltuples::bigint, pg_size_pretty(pg_total_relation_size(oid)) FROM pg_class WHERE relname IN ('orders', 'lineitem');
