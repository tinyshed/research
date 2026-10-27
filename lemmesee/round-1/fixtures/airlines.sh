#!/bin/sh
# A development fixture: Postgres Professional's airlines demo, version of
# 1 September 2025, three months (MIT), fetched into lemmesee/corpus/,
# checked against hashes.txt and loaded into the compose server's
# PostgreSQL as the database `demo`, schema `bookings`, with the round's
# read-only role. Its rules are tuned by looking at it, so it is never a
# pilot fixture.
#
#   sh lemmesee/round-1/fixtures/airlines.sh
set -eu

here=$(cd "$(dirname "$0")" && (pwd -W 2>/dev/null || pwd))
round=$(dirname "$here")
corpus=$(cd "$round/.." && (pwd -W 2>/dev/null || pwd))/corpus
file=demo-20250901-3m.sql.gz
dir=$corpus/airlines/20250901-3m

mkdir -p "$dir"
[ -f "$dir/$file" ] || curl -sSfL -o "$dir/$file" "https://edu.postgrespro.ru/$file"
(cd "$corpus" && grep airlines/ "$here/hashes.txt" | sha256sum -c -)

compose="docker compose -f $round/compose.yaml"
# The dump drops `demo` before it creates it, so one must exist
$compose exec -T postgres psql -q -U postgres -c "DROP DATABASE IF EXISTS demo" -c "CREATE DATABASE demo" >/dev/null
gzip -dc "$dir/$file" | $compose exec -T postgres psql -q -U postgres >/dev/null
$compose exec -T postgres psql -q -v ON_ERROR_STOP=1 -U postgres -d demo <<'SQL'
CREATE EXTENSION IF NOT EXISTS pg_stat_statements;
DO $$ BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'lemmesee') THEN
    CREATE ROLE lemmesee LOGIN PASSWORD 'lemmesee';
  END IF;
END $$;
GRANT CONNECT ON DATABASE demo TO lemmesee;
GRANT USAGE ON SCHEMA bookings TO lemmesee;
GRANT SELECT ON ALL TABLES IN SCHEMA bookings TO lemmesee;
ANALYZE;
SQL
echo "airlines loaded: postgres fresh"
