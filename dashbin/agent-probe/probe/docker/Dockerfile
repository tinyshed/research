# the probe's static binary, built by build.sh into this folder, with tc to slow an agent's link
FROM alpine:3.22
RUN apk add --no-cache iproute2
COPY agent-probe /usr/local/bin/agent-probe
