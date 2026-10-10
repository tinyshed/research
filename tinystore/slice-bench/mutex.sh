#!/bin/sh
# A diagnosis beside the round, and no round of its own: reads by a few
# callers with SQLite's mutexes as they are, which park a thread that finds
# one held, and made to spin first. A shim gives every pthread_mutex_init
# without attributes the adaptive kind, and glibc's tunable sets how long it
# spins. Nothing of TinyStore is rebuilt.
#
#   mutex.sh <tree> <out>     in golang:1.27, the tree on the measurements' volume
#
# It writes a line of JSON a run to <out>.
set -eu

tree=$1
out=$2
store=$tree/run-mutex

cat > /tmp/adaptive.c <<'EOF'
#define _GNU_SOURCE
#include <dlfcn.h>
#include <pthread.h>

int pthread_mutex_init(pthread_mutex_t *mutex, const pthread_mutexattr_t *given) {
  static int (*real)(pthread_mutex_t *, const pthread_mutexattr_t *);
  if (!real) real = dlsym(RTLD_NEXT, "pthread_mutex_init");
  if (given) return real(mutex, given);
  pthread_mutexattr_t adaptive;
  pthread_mutexattr_init(&adaptive);
  pthread_mutexattr_settype(&adaptive, PTHREAD_MUTEX_ADAPTIVE_NP);
  int made = real(mutex, &adaptive);
  pthread_mutexattr_destroy(&adaptive);
  return made;
}
EOF
gcc -O2 -shared -fPIC -o /tmp/adaptive.so /tmp/adaptive.c -ldl

# run <case> <callers> <mutex's name> [environment...]
run() {
	case=$1 callers=$2 mutex=$3
	shift 3
	rm -rf "$store" && mkdir -p "$store"
	rate=$(env "$@" "$tree/bin/rust-slice" --dir "$store" --case "$case" --callers "$callers" --seconds 3 |
		sed -e 's/.*"per_second":\([0-9.]*\).*/\1/')
	echo "{\"case\": \"$case\", \"callers\": $callers, \"mutex\": \"$mutex\", \"per_second\": $rate}" | tee -a "$out"
}

: > "$out"
for pass in 1 2; do
	for callers in 1 2 3 4 6 8 16; do
		run kv-get "$callers" parks NOTHING=1
		run kv-get "$callers" spins-100 LD_PRELOAD=/tmp/adaptive.so
		run kv-get "$callers" spins-1000 LD_PRELOAD=/tmp/adaptive.so GLIBC_TUNABLES=glibc.pthread.mutex_spin_count=1000
	done
done
for callers in 2 4; do
	run sql-point "$callers" parks NOTHING=1
	run sql-point "$callers" spins-1000 LD_PRELOAD=/tmp/adaptive.so GLIBC_TUNABLES=glibc.pthread.mutex_spin_count=1000
done
for spins in 30 300 3000 30000; do
	run kv-get 4 "spins-$spins" LD_PRELOAD=/tmp/adaptive.so "GLIBC_TUNABLES=glibc.pthread.mutex_spin_count=$spins"
done
rm -rf "$store"
