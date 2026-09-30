#!/bin/sh
# The kv round through the Bun and Python SDKs, beside Redis through each
# language's own client, into $OUT/sdk-kv.json. Run from bench/compare inside
# the image Dockerfile.sdk builds, after run.sh:
#
#   docker run --rm -v <research>:/src -v tinystore-compare-data:/data \
#     -e GOWORK=off -e GOFLAGS=-buildvcs=false tinystore-compare-sdk sh sdk/sdk.sh
#
# Each language measures TinyStore through its sidecar (tinystore) and through
# a remote server on loopback TCP with a token (tinystore-server), and Redis on
# its Unix socket (redis) and on loopback TCP (redis-tcp), appendfsync always.
# REPEATS (3) and SECONDS_A_STAGE (5) as run.sh takes them.
set -u

date=$(date -u +%Y-%m-%d)
out="${OUT:-results/$date}"
mkdir -p "$out"
if [ -s "$out/sdk-kv.json" ]; then
	echo "$(date -u +%H:%M:%S) have $out/sdk-kv.json" >> "$out/progress.txt"
	exit 0
fi
go build -C ../../source/cmd/tinystore -o /tmp/tinystore . || exit 1
export TINYSTORE_BIN=/tmp/tinystore
runs=$(mktemp -d)
echo "$(date -u +%H:%M:%S) start sdk kv" >> "$out/progress.txt"

# one runs a language's script against a contender in a directory of its own,
# with whatever server that contender needs started and stopped around it
one() {
	language=$1 contender=$2 repeat=$3
	dir=$(mktemp -d /data/sdk-XXXXXX)
	service=""
	case $contender in
	tinystore-server)
		token=$(head -c 32 /dev/urandom | base64 | tr '+/' '-_' | tr -d '=')
		echo "data $token" > "$dir.tokens"
		port=$((20000 + $(od -An -N2 -tu2 /dev/urandom) % 20000))
		/tmp/tinystore serve --dir "$dir" --listen "tcp://127.0.0.1:$port" --tokens "$dir.tokens" 2>>"$runs/server.log" &
		service=$!
		export TINYSTORE_URL="tcp://127.0.0.1:$port" TINYSTORE_TOKEN="$token"
		sleep 1
		;;
	redis | redis-tcp)
		if [ "$contender" = redis ]; then
			listen="--port 0 --unixsocket $dir/redis.sock --unixsocketperm 700"
			export REDIS_URL="unix://$dir/redis.sock"
			[ "$language" = bun ] && export REDIS_URL="redis+unix://$dir/redis.sock"
		else
			port=$((20000 + $(od -An -N2 -tu2 /dev/urandom) % 20000))
			listen="--port $port --bind 127.0.0.1"
			export REDIS_URL="redis://127.0.0.1:$port"
		fi
		# shellcheck disable=SC2086 # the listen flags are words
		redis-server $listen --dir "$dir" --appendonly yes --appendfsync always --save "" --daemonize no \
			>>"$runs/redis.log" 2>&1 &
		service=$!
		sleep 1
		;;
	esac
	export SERVICE_PID="$service"
	case $language in
	bun) bun sdk/kv.ts "$contender" "$dir" "${SECONDS_A_STAGE:-5}" ;;
	python) python3 sdk/kv.py "$contender" "$dir" "${SECONDS_A_STAGE:-5}" ;;
	esac > "$runs/$language-$contender-$repeat.json" 2>>"$runs/errors.log" ||
		echo "$language $contender repeat $repeat failed" >> "$runs/failed.txt"
	if [ -n "$service" ]; then
		kill -INT "$service" 2>/dev/null
		wait "$service" 2>/dev/null
	fi
	# a sidecar the SDK started leaves once idle; this one was told never to
	pkill -INT -f "$dir" 2>/dev/null
	sleep 1
	rm -rf "$dir" "$dir.tokens"
}

for repeat in $(seq 1 "${REPEATS:-3}"); do
	for language in bun python; do
		for contender in tinystore tinystore-server redis redis-tcp; do
			one "$language" "$contender" "$repeat"
		done
	done
done

python3 - "$runs" "$out/sdk-kv.json" <<'EOF'
import json, pathlib, sys, datetime
runs_dir, out = pathlib.Path(sys.argv[1]), sys.argv[2]
runs, failed = [], []
for path in sorted(runs_dir.glob("*.json")):
    text = path.read_text().strip()
    if not text:
        continue
    run = json.loads(text.splitlines()[-1])
    run["repeat"] = int(path.stem.rsplit("-", 1)[1])
    runs.append(run)
failures = runs_dir / "failed.txt"
if failures.exists():
    failed = failures.read_text().split("\n")
    failed = [f for f in failed if f]
round_ = {"engine": "sdk-kv", "started": datetime.datetime.now(datetime.UTC).isoformat(), "runs": runs}
if failed:
    round_["failed"] = failed
pathlib.Path(out).write_text(json.dumps(round_, indent=2))
EOF
cp "$runs/errors.log" "$out/sdk-errors.log" 2>/dev/null
if [ -s "$runs/failed.txt" ]; then
	echo "$(date -u +%H:%M:%S) failed: sdk kv, $(wc -l < "$runs/failed.txt") runs" | tee -a "$out/failures.txt" >> "$out/progress.txt"
else
	echo "$(date -u +%H:%M:%S) done $out/sdk-kv.json" >> "$out/progress.txt"
fi
