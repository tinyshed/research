# The comparison image with Bun and Python, for sdk/sdk.sh. Built on the one
# this directory's parent Dockerfile makes, with the Bun fetch-bun.sh leaves:
#
#   sh tinystore/bench/compare/sdk/fetch-bun.sh
#   docker build -t tinystore-compare-sdk tinystore/bench/compare/sdk
FROM tinystore-compare
RUN apt-get update && apt-get install -y --no-install-recommends python3 python3-pip procps \
	&& rm -rf /var/lib/apt/lists/*
COPY bun /usr/local/bin/bun
RUN chmod 755 /usr/local/bin/bun && bun --version | grep -qx 1.4.2
RUN pip3 install --break-system-packages redis==6.4.0
