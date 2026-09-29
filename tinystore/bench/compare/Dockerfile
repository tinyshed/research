# The bench's machine: Go, and the servers the comparison runs beside the
# libraries. A report names this image's digest and the versions it printed.
FROM golang:1.27
RUN apt-get update \
 && apt-get install -y --no-install-recommends redis-server strace \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /src/tinystore/bench/compare
