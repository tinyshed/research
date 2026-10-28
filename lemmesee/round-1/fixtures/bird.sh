#!/bin/sh
# A fixture from BIRD's training or development databases (bird-bench.github.io,
# CC BY-SA 4.0), fetched once into lemmesee/corpus/bird/, checked against
# hashes.txt when first unpacked, and loaded from its SQLite file into the
# compose server's PostgreSQL as the database bird_<name> by sqlite.py, with
# the round's read-only role. Databases CTU already serves, and the pilot
# fixtures' own, are not loaded from here.
#
#   sh lemmesee/round-1/fixtures/bird.sh train|dev <database>
set -eu

part=$1
db=$2
here=$(cd "$(dirname "$0")" && (pwd -W 2>/dev/null || pwd))
round=$(dirname "$here")
corpus=$(cd "$round/.." && (pwd -W 2>/dev/null || pwd))/corpus
dir=$corpus/bird
compose="docker compose -f $round/compose.yaml"

mkdir -p "$dir/raw/$part"
[ -f "$dir/$part.zip" ] ||
  curl -sSfL -o "$dir/$part.zip" "https://bird-bench.oss-cn-beijing.aliyuncs.com/$part.zip"
inner=$dir/raw/$part/${part}_databases.zip
if [ ! -f "$inner" ]; then
  (cd "$corpus" && grep " bird/$part.zip\$" "$here/hashes.txt" | sha256sum -c -)
  # train.zip keeps it under train/, dev.zip under a dated folder
  unzip -o -q -j "$dir/$part.zip" "*/${part}_databases.zip" -x "__MACOSX/*" -d "$dir/raw/$part"
fi
src=$dir/raw/$part/${part}_databases/$db/$db.sqlite
[ -f "$src" ] || (cd "$dir/raw/$part" && unzip -o -q "$inner" "${part}_databases/$db/$db.sqlite")

name=bird_$(echo "$db" | tr 'A-Z' 'a-z')
$compose exec -T postgres psql -q -U postgres -c "DROP DATABASE IF EXISTS $name" -c "CREATE DATABASE $name" >/dev/null
python -I "$here/sqlite.py" "$src" | $compose exec -T postgres psql -q -v ON_ERROR_STOP=1 -U postgres -d "$name" >/dev/null
$compose exec -T postgres psql -q -v ON_ERROR_STOP=1 -U postgres -d "$name" <<SQL
CREATE EXTENSION IF NOT EXISTS pg_stat_statements;
DO \$\$ BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'lemmesee') THEN
    CREATE ROLE lemmesee LOGIN PASSWORD 'lemmesee';
  END IF;
END \$\$;
GRANT CONNECT ON DATABASE $name TO lemmesee;
GRANT USAGE ON SCHEMA public TO lemmesee;
GRANT SELECT ON ALL TABLES IN SCHEMA public TO lemmesee;
ANALYZE;
SQL
echo "$name loaded"
