#!/bin/sh
# A check fixture: Northwind, Microsoft's sample shop, as pthom/northwind_psql
# ports it to PostgreSQL (its repository states no licence of its own),
# fetched at a pinned commit into lemmesee/corpus/, checked against
# hashes.txt and loaded into the compose server's PostgreSQL as the database
# `northwind`, with the round's read-only role. The rules are measured on it
# once before anything is changed by looking at it; after that it is a
# development fixture, and never a pilot fixture.
#
#   sh lemmesee/round-1/fixtures/northwind.sh
set -eu

here=$(cd "$(dirname "$0")" && (pwd -W 2>/dev/null || pwd))
round=$(dirname "$here")
corpus=$(cd "$round/.." && (pwd -W 2>/dev/null || pwd))/corpus
commit=cd0ef28d66369fbe177778e604e4be0f153c9e5c
dir=$corpus/northwind

mkdir -p "$dir"
[ -f "$dir/northwind.sql" ] ||
  curl -sSfL -o "$dir/northwind.sql" "https://raw.githubusercontent.com/pthom/northwind_psql/$commit/northwind.sql"
(cd "$corpus" && grep northwind/ "$here/hashes.txt" | sha256sum -c -)

compose="docker compose -f $round/compose.yaml"
$compose exec -T postgres psql -q -U postgres -c "DROP DATABASE IF EXISTS northwind" -c "CREATE DATABASE northwind" >/dev/null
$compose exec -T postgres psql -q -U postgres -d northwind < "$dir/northwind.sql" >/dev/null
$compose exec -T postgres psql -q -v ON_ERROR_STOP=1 -U postgres -d northwind <<'SQL'
CREATE EXTENSION IF NOT EXISTS pg_stat_statements;
DO $$ BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'lemmesee') THEN
    CREATE ROLE lemmesee LOGIN PASSWORD 'lemmesee';
  END IF;
END $$;
GRANT CONNECT ON DATABASE northwind TO lemmesee;
GRANT USAGE ON SCHEMA public TO lemmesee;
GRANT SELECT ON ALL TABLES IN SCHEMA public TO lemmesee;
ANALYZE;
SQL
echo "northwind loaded: postgres fresh"
