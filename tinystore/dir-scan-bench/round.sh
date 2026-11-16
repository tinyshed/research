#!/bin/sh
# dir-scan: what a directory scan of TinyStore's planned external directories
# costs, on NTFS natively and on ext4 in a container. From the repository's
# root, in Git Bash on Windows:
#
#   sh tinystore/dir-scan-bench/round.sh windows   # release build, run.sh on NTFS, warm
#   sh tinystore/dir-scan-bench/round.sh tools     # the Linux binary, in rust:1.99.1, on a volume
#   sh tinystore/dir-scan-bench/round.sh check     # a mount and a link inside a tree, then std's syscalls
#   sh tinystore/dir-scan-bench/round.sh linux     # run.sh on ext4 of a Docker volume, warm and cold
#   sh tinystore/dir-scan-bench/round.sh summary   # the tables, from the raw lines
#   sh tinystore/dir-scan-bench/round.sh clean     # the volumes
#
# The raw lines go to tinystore/reports/data/dir-scan-2026-11-16/, or DATA's.
set -eu
export MSYS_NO_PATHCONV=1
repo=$(pwd -W 2>/dev/null || pwd)
data=${DATA:-tinystore/reports/data/dir-scan-2026-11-16}
mkdir -p "$data"
volumes="-v $repo:/src -v dir-scan-cargo:/usr/local/cargo/registry -v dir-scan-target:/target \
  -v dir-scan-data:/data -e CARGO_TARGET_DIR=/target"
image=rust:1.99.1
bin=/target/release/dir-scan-bench

case "${1:-}" in
windows)
  (cd tinystore/dir-scan-bench && rustc --version && cargo build --release 2>&1 | tail -1) >&2
  mkdir -p tinystore/dir-scan-bench/fixtures
  BIN=tinystore/dir-scan-bench/target/release/dir-scan-bench.exe ROOT=tinystore/dir-scan-bench/fixtures COLD=0 \
    sh tinystore/dir-scan-bench/run.sh >> "$data/windows.jsonl"
  ;;
tools)
  docker run --rm $volumes -w /src/tinystore/dir-scan-bench $image \
    sh -c 'rustc --version && cargo build --release 2>&1 | tail -1'
  ;;
check)
  docker run --rm --privileged $volumes $image sh -c "
    set -e; rm -rf /data/check /data/check.db*; $bin gen /data/check lib:2000 1 >/dev/null
    $bin scan /data/check /data/check.db listed
    one=\$(ls -d /data/check/*/ | head -1); two=\$(ls -d /data/check/*/ | sed -n 2p)
    mount -t tmpfs none \"\$one\"; ln -s /etc \"\$two/link\"
    $bin scan /data/check /data/check.db 'a tmpfs over a folder and a link'
    umount \"\$one\"; rm \"\$two/link\"
    $bin scan /data/check /data/check.db 'both gone'
    apt-get -qq update >/dev/null && apt-get -qq install -y strace >/dev/null
    strace -c -f -o /tmp/walk.txt $bin walk /data/check strace >/dev/null; head -14 /tmp/walk.txt
    rm -rf /data/check /data/check.db*" | tee "$data/check-linux.txt"
  ;;
linux)
  docker info --format '{"docker":"{{.ServerVersion}}","kernel":"{{.KernelVersion}}","cpus":{{.NCPU}},"memory":{{.MemTotal}}}' \
    >> "$data/environment.jsonl"
  docker run --rm --privileged $volumes $image sh -c \
    "df -T /data | tail -1 >&2; mkdir -p /data/fixtures; BIN=$bin ROOT=/data/fixtures COLD=1 sh /src/tinystore/dir-scan-bench/run.sh" \
    >> "$data/linux.jsonl"
  ;;
summary)
  python -I tinystore/dir-scan-bench/summary.py "$data" > "$data/summary.md"
  cat "$data/summary.md"
  ;;
clean)
  docker volume rm dir-scan-cargo dir-scan-target dir-scan-data
  ;;
*)
  echo "round.sh windows|tools|check|linux|summary|clean" >&2
  exit 2
  ;;
esac
