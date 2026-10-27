#!/bin/sh
# A development fixture: Ergast's Formula 1 database as CTU's relational
# repository serves it (relational.fel.cvut.cz, database ErgastF1, MariaDB
# 10.6; the data from ergast.com), dumped once into lemmesee/corpus/, checked
# against hashes.txt, and loaded into the compose server's MySQL with the
# round's read-only role and a histogram on every column, so that B1 on
# MySQL has statistics to read. Its rules are tuned by looking at it, so it
# is never a pilot fixture.
#
#   sh lemmesee/round-1/fixtures/ergastf1.sh
set -eu

here=$(cd "$(dirname "$0")" && (pwd -W 2>/dev/null || pwd))
round=$(dirname "$here")
corpus=$(cd "$round/.." && (pwd -W 2>/dev/null || pwd))/corpus
dir=$corpus/ergastf1
compose="docker compose -f $round/compose.yaml"

mkdir -p "$dir"
if [ ! -f "$dir/ErgastF1.sql" ]; then
  # CTU's public read-only account, as its site gives it
  $compose exec -T -e MYSQL_PWD=ctu-relational mysql mysqldump -h relational.fel.cvut.cz -P 3306 -u guest \
    --column-statistics=0 --single-transaction --skip-lock-tables --no-tablespaces ErgastF1 > "$dir/ErgastF1.sql"
fi
(cd "$corpus" && grep ergastf1/ "$here/hashes.txt" | sha256sum -c -)

my="$compose exec -T -e MYSQL_PWD=lemmesee-admin mysql mysql -uroot"
$my -e "DROP DATABASE IF EXISTS ErgastF1; CREATE DATABASE ErgastF1"
$my ErgastF1 < "$dir/ErgastF1.sql"
$my <<'SQL'
CREATE USER IF NOT EXISTS 'lemmesee'@'%' IDENTIFIED BY 'lemmesee';
GRANT SELECT ON ErgastF1.* TO 'lemmesee'@'%';
SQL
# information_schema keeps a table's row estimate for a day: queried while a
# table was still empty, it says 0 until then. ANALYZE TABLE refreshes it.
$my -N -e "SELECT CONCAT('ANALYZE TABLE \`', TABLE_NAME, '\`;') FROM information_schema.TABLES
           WHERE TABLE_SCHEMA = 'ErgastF1'" | $my ErgastF1 >/dev/null
# A histogram on every column of every table: the state fixtures.md calls
# `histograms`
$my -N -e "SELECT CONCAT('ANALYZE TABLE \`', TABLE_NAME, '\` UPDATE HISTOGRAM ON ',
             GROUP_CONCAT(CONCAT('\`', COLUMN_NAME, '\`')), ' WITH 100 BUCKETS;')
           FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = 'ErgastF1'
           AND DATA_TYPE NOT IN ('json', 'geometry') GROUP BY TABLE_NAME" | $my ErgastF1 >/dev/null
echo "ergastf1 loaded: mysql histograms"
