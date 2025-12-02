# Build stage
FROM rust:1.85-slim as builder

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build release binary
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install CA certificates for HTTPS
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/zoldyq /usr/local/bin/zoldyq

# Create non-root user
RUN useradd -m -u 1000 zoldyq && \
    chown -R zoldyq:zoldyq /app

USER zoldyq

# Expose port
EXPOSE 9000

# Set environment variables
ENV ZOLDYQ_HOST=0.0.0.0
ENV ZOLDYQ_PORT=9000
ENV ZOLDYQ_DEFAULT_QUEUE_CAPACITY=100000
ENV ZOLDYQ_MAX_QUEUES=100
ENV ZOLDYQ_LOG_LEVEL=info

# Run the binary
CMD ["zoldyq"]

