# The bench's machine: Go, and the servers the comparison runs beside the
# libraries. A report names this image's digest and the versions it printed.
FROM golang:1.27
RUN apt-get update \
 && apt-get install -y --no-install-recommends redis-server postgresql strace \
 && rm -rf /var/lib/apt/lists/*
ARG VICTORIA=v1.153.0
ARG VICTORIA_SHA256=1b495bde563825cf83dc7c0425a9d8fa03e7214858e0949f9177efc6bb1f8bfc
RUN curl -fsSL -o /tmp/vm.tgz \
      "https://github.com/VictoriaMetrics/VictoriaMetrics/releases/download/${VICTORIA}/victoria-metrics-linux-amd64-${VICTORIA}.tar.gz" \
 && echo "${VICTORIA_SHA256}  /tmp/vm.tgz" | sha256sum -c - \
 && tar -xzf /tmp/vm.tgz -C /usr/local/bin victoria-metrics-prod \
 && rm /tmp/vm.tgz
WORKDIR /src/tinystore/bench/compare
