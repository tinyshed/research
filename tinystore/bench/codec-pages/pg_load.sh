#!/bin/sh
# Load datasets' kept bodies into PostgreSQL 18 and divide what it stores.
#
#   sh pg_load.sh <dataset> [<dataset> ...]
#
# For each dataset: pg_prepare.py writes a COPY file a bodies file of
# /work/out/<dataset>/, pg_sql.py the psql script, and psql runs it in the
# container `codec-pages-pg` (postgres:18, autovacuum off, trust
# authentication on its own socket, no published port, data in the volume
# codec-pages-pg), its size lines to /work/pg/out/<dataset>.sizes. The COPY
# files are deleted afterwards.
set -eu
export MSYS_NO_PATHCONV=1
hostpath() { if command -v cygpath >/dev/null 2>&1; then cygpath -m "$1"; else printf '%s' "$1"; fi; }
here=$(cd "$(dirname "$0")" && pwd)
repo=$(hostpath "$(cd "$here/../../.." && pwd)")
GO=golang:1.27@sha256:2920bb124f1a2e12a1d39f0cab756ae0068441ee1da7655dec7f5e374944095b
PG=postgres:18@sha256:edb0c08dc33befd0262ab3b5cf1c4f3f8cfd8b09e88c05b45adee29d6761ddb6

if ! docker ps --format '{{.Names}}' | grep -qx codec-pages-pg; then
	docker run -d --name codec-pages-pg -e POSTGRES_HOST_AUTH_METHOD=trust \
		-v codec-pages-pg:/var/lib/postgresql -v codec-pages-work:/work \
		"$PG" -c autovacuum=off -c fsync=off -c full_page_writes=off >/dev/null
	until docker exec codec-pages-pg pg_isready -U postgres -q; do sleep 1; done
	sleep 2
fi
docker exec codec-pages-pg sh -c 'mkdir -p /work/pg/out /work/pg/copy && chmod 777 /work/pg/out /work/pg/copy'

for ds in "$@"; do
	docker run --rm -v codec-pages-work:/work -v "$repo:/src:ro" $GO sh -c "
		python3 -I /src/tinystore/bench/codec-pages/pg_prepare.py /work/pg/copy /work/out/$ds/*.bodies > /dev/null &&
		python3 -I /src/tinystore/bench/codec-pages/pg_sql.py $ds > /work/pg/out/$ds.sql &&
		chmod 644 /work/pg/copy/${ds}__*.tsv"
	docker exec codec-pages-pg psql -U postgres -q -v ON_ERROR_STOP=1 -At -F '	' \
		-f "/work/pg/out/$ds.sql" -o "/work/pg/out/$ds.sizes"
	docker exec codec-pages-pg sh -c "rm -f /work/pg/copy/${ds}__*.tsv"
	echo "$(date -u +%H:%M:%S) $ds loaded" >&2
done
