#!/bin/sh
# Downloads the VictoriaMetrics release the Dockerfile checks and copies in.
set -eu
curl -fsSL -o victoria-metrics.tar.gz \
	https://github.com/VictoriaMetrics/VictoriaMetrics/releases/download/v1.153.0/victoria-metrics-linux-amd64-v1.153.0.tar.gz
