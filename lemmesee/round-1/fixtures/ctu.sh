#!/bin/sh
# A check fixture from CTU's relational repository (relational.fel.cvut.cz,
# MariaDB 10.6; each database's source and licence on its page there),
# dumped once into lemmesee/corpus/, checked against hashes.txt, and loaded
# into the compose server's MySQL with the round's read-only role and a
# histogram on every column, as ergastf1.sh loads ErgastF1. The rules are
# measured on it once before anything is changed by looking at it; after
# that it is a development fixture, and never a pilot fixture.
#
#   sh lemmesee/round-1/fixtures/ctu.sh <database>     e.g. ccs, lahman_2014
set -eu

db=$1
here=$(cd "$(dirname "$0")" && (pwd -W 2>/dev/null || pwd))
round=$(dirname "$here")
corpus=$(cd "$round/.." && (pwd -W 2>/dev/null || pwd))/corpus
dir=$corpus/ctu
compose="docker compose -f $round/compose.yaml"

mkdir -p "$dir"
if [ ! -f "$dir/$db.sql" ]; then
  # CTU's public read-only account, as its site gives it
  $compose exec -T -e MYSQL_PWD=ctu-relational mysql mysqldump -h relational.fel.cvut.cz -P 3306 -u guest \
    --column-statistics=0 --single-transaction --skip-lock-tables --no-tablespaces --skip-dump-date "$db" > "$dir/$db.sql"
fi
(cd "$corpus" && grep " ctu/$db.sql" "$here/hashes.txt" | sha256sum -c -)

my="$compose exec -T -e MYSQL_PWD=lemmesee-admin mysql mysql -uroot"
$my -e "DROP DATABASE IF EXISTS \`$db\`; CREATE DATABASE \`$db\`"
# MariaDB lets a foreign key point at a key that is not unique, as
# lahman_2014's awards point at managers; MySQL 8.4 refuses one unless told
{ echo "SET SESSION restrict_fk_on_non_standard_key = OFF;"; cat "$dir/$db.sql"; } | $my "$db"
$my <<SQL
CREATE USER IF NOT EXISTS 'lemmesee'@'%' IDENTIFIED BY 'lemmesee';
GRANT SELECT ON \`$db\`.* TO 'lemmesee'@'%';
SQL
$my -N -e "SELECT CONCAT('ANALYZE TABLE \`', TABLE_NAME, '\`;') FROM information_schema.TABLES
           WHERE TABLE_SCHEMA = '$db'" | $my "$db" >/dev/null
# GROUP_CONCAT stops at 1 024 bytes unless told, and PremierLeague's actions
# have more columns than that
$my -N -e "SET SESSION group_concat_max_len = 1000000;
           SELECT CONCAT('ANALYZE TABLE \`', TABLE_NAME, '\` UPDATE HISTOGRAM ON ',
             GROUP_CONCAT(CONCAT('\`', COLUMN_NAME, '\`')), ' WITH 100 BUCKETS;')
           FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = '$db'
           AND DATA_TYPE NOT IN ('json', 'geometry') GROUP BY TABLE_NAME" | $my "$db" >/dev/null
echo "$db loaded: mysql histograms"
