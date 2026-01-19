# Build stage
FROM rust:1.77-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 harbor

# Copy binary from builder
COPY --from=builder /app/target/release/harbor-bootstrap /usr/local/bin/

# Set ownership
RUN chown harbor:harbor /usr/local/bin/harbor-bootstrap

# Switch to non-root user
USER harbor

# Expose default ports (TCP and QUIC)
EXPOSE 9000/tcp
EXPOSE 9001/udp

# Set entrypoint
ENTRYPOINT ["harbor-bootstrap"]

# Default arguments
CMD ["--port", "9000"]
