# Harbor Bootstrap Node - Docker Image
#
# Build:
#   docker build -t harbor-bootstrap .
#
# Run:
#   docker run -p 9000:9000 harbor-bootstrap --external-ip YOUR_PUBLIC_IP
#
# Or with docker-compose (see docker-compose.yml)

FROM rust:1.75-slim as builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy source code
COPY Cargo.toml ./
COPY src ./src

# Build release binary
RUN cargo build --release

# Runtime stage - use minimal image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/harbor-bootstrap /usr/local/bin/

# Expose default port
EXPOSE 9000

# Run the bootstrap node
ENTRYPOINT ["harbor-bootstrap"]
CMD ["--port", "9000"]
