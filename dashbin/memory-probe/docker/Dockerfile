FROM rust:1.99-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends musl-tools cmake && rm -rf /var/lib/apt/lists/* \
 && rustup target add x86_64-unknown-linux-musl
