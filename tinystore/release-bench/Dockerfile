# The release round's machine: Bun, TinyStore as npm publishes it, and every
# service a small application runs without it, each at its vendor's release.
# A report names this image's digest and the versions `versions.sh` printed.

# MinIO publishes no binaries since October 2025, so its last release is built
# from its tag.
FROM golang:1.27-trixie AS minio
RUN CGO_ENABLED=0 GOFLAGS=-trimpath go install github.com/minio/minio@RELEASE.2025-10-15T17-29-55Z

FROM redis:8.10.2-trixie AS redis
FROM valkey/valkey:9.1.2-trixie AS valkey
FROM victoriametrics/victoria-metrics:v1.154.1 AS victoria
FROM prom/prometheus:v3.15.1 AS prometheus
FROM grafana/loki:3.7.9 AS loki
FROM oven/bun:1.4.2-debian AS bun

FROM postgres:18-trixie
RUN apt-get update \
 && apt-get install -y --no-install-recommends python3 procps strace ca-certificates unzip \
 && rm -rf /var/lib/apt/lists/*
COPY --from=minio /go/bin/minio /usr/local/bin/minio
COPY --from=redis /usr/local/bin/redis-server /usr/local/bin/redis-server
COPY --from=redis /usr/local/bin/redis-cli /usr/local/bin/redis-cli
COPY --from=valkey /usr/local/bin/valkey-server /usr/local/bin/valkey-server
COPY --from=victoria /victoria-metrics-prod /usr/local/bin/victoria-metrics
COPY --from=prometheus /bin/prometheus /usr/local/bin/prometheus
COPY --from=prometheus /bin/promtool /usr/local/bin/promtool
COPY --from=loki /usr/bin/loki /usr/local/bin/loki
COPY --from=bun /usr/local/bin/bun /usr/local/bin/bun
ENV PATH=/usr/lib/postgresql/18/bin:$PATH
WORKDIR /bench
