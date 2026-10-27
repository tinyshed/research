#!/bin/sh
# P1, Chinook 1.4.5 (MIT), at a pinned commit: fetch its two scripts into
# lemmesee/corpus/, check them against hashes.txt, and load them into the
# compose servers with the round's read-only role.
#
#   sh lemmesee/round-1/fixtures/chinook.sh            fetch, check, load
#   STATS=never sh lemmesee/round-1/fixtures/chinook.sh  PostgreSQL left unanalysed
set -eu

# pwd -W gives Git Bash's paths as Windows writes them, which docker.exe reads
here=$(cd "$(dirname "$0")" && (pwd -W 2>/dev/null || pwd))
round=$(dirname "$here")
corpus=$(cd "$round/.." && (pwd -W 2>/dev/null || pwd))/corpus
commit=7f67772503d71ba90f19283c38e93923addb43fa
dir=$corpus/chinook/$commit
pg=Chinook_PostgreSql_AutoIncrementPKs.sql
my=Chinook_MySql_AutoIncrementPKs.sql
stats=${STATS:-fresh}

mkdir -p "$dir"
for f in $pg $my; do
  [ -f "$dir/$f" ] || curl -sSfL -o "$dir/$f" \
    "https://raw.githubusercontent.com/lerocha/chinook-database/$commit/ChinookDatabase/DataSources/$f"
done
(cd "$corpus" && grep chinook/ "$here/hashes.txt" | sha256sum -c -)

compose="docker compose -f $round/compose.yaml"

# PostgreSQL: the script drops and creates chinook_auto_increment itself.
# With STATS=never autovacuum is off before the rows arrive, so no
# statistics exist until someone analyses.
if [ "$stats" = never ]; then
  $compose exec -T postgres psql -q -U postgres -c "ALTER SYSTEM SET autovacuum = off" -c "SELECT pg_reload_conf()" >/dev/null
fi
$compose exec -T postgres psql -q -v ON_ERROR_STOP=1 -U postgres <"$dir/$pg" >/dev/null
$compose exec -T postgres psql -q -v ON_ERROR_STOP=1 -U postgres -d chinook_auto_increment <<'SQL'
CREATE EXTENSION IF NOT EXISTS pg_stat_statements;
DO $$ BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'lemmesee') THEN
    CREATE ROLE lemmesee LOGIN PASSWORD 'lemmesee';
  END IF;
END $$;
GRANT CONNECT ON DATABASE chinook_auto_increment TO lemmesee;
GRANT USAGE ON SCHEMA public TO lemmesee;
GRANT SELECT ON ALL TABLES IN SCHEMA public TO lemmesee;
SQL
if [ "$stats" = fresh ]; then
  $compose exec -T postgres psql -q -U postgres -d chinook_auto_increment -c "ANALYZE"
fi

# MySQL: the script creates Chinook_AutoIncrement itself; no histogram is
# made, the state the round calls none.
$compose exec -T -e MYSQL_PWD=lemmesee-admin mysql mysql -uroot -e "DROP DATABASE IF EXISTS Chinook_AutoIncrement"
$compose exec -T -e MYSQL_PWD=lemmesee-admin mysql mysql -uroot <"$dir/$my"
$compose exec -T -e MYSQL_PWD=lemmesee-admin mysql mysql -uroot <<'SQL'
CREATE USER IF NOT EXISTS 'lemmesee'@'%' IDENTIFIED BY 'lemmesee';
GRANT SELECT ON Chinook_AutoIncrement.* TO 'lemmesee'@'%';
SQL

echo "chinook loaded: postgres $stats, mysql none"
