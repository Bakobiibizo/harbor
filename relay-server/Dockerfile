# syntax=docker/dockerfile:1

FROM rust:1-bullseye AS builder
WORKDIR /app
COPY . .
# Use incremental-less release build for smaller image and faster startup
RUN cargo build --release

FROM debian:12-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 10001 relay

WORKDIR /app
COPY --from=builder /app/target/release/harbor-relay /usr/local/bin/harbor-relay

# Persist identity key and any future state
RUN mkdir -p /var/lib/harbor-relay \
    && chown -R relay:relay /var/lib/harbor-relay

USER relay
ENV RUST_LOG=info \
    IDENTITY_KEY_PATH=/var/lib/harbor-relay/id.key

EXPOSE 4001/tcp
EXPOSE 4001/udp

ENTRYPOINT ["/usr/local/bin/harbor-relay"]
