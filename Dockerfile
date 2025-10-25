# =============================================================================
# Multi-stage Docker build for HFT Arbitrage Bot
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1: Builder (Compile Rust application with all dependencies)
# -----------------------------------------------------------------------------
FROM rustlang/rust:nightly-slim AS builder

# Set build arguments
ARG RUST_BUILD_PROFILE=release
ARG TARGETARCH

# Install system dependencies for compilation
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    build-essential \
    cmake \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy manifest files first for dependency caching
COPY Cargo.toml Cargo.lock* ./


# Create dummy main.rs to cache dependencies
# Using cargo +nightly for edition2024 support
RUN mkdir src && \
    echo "fn main() {println!(\"if you see this, the build broke\")}" > src/main.rs && \
    cargo +nightly build --release && \
    rm -rf src target/release/.fingerprint/hft-arbitrage-bot-*

# Copy source code and migrations
COPY src ./src
COPY migrations ./migrations

# Build the application with optimizations using nightly
RUN cargo +nightly build --release && \
    strip target/release/hft-arbitrage-bot

# -----------------------------------------------------------------------------
# Stage 2: Runtime (Minimal production image)
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    # SSL certificates for HTTPS connections
    ca-certificates \
    # OpenSSL for TLS
    libssl3 \
    # PostgreSQL client library
    libpq5 \
    # Curl for health checks
    curl \
    # ONNX Runtime dependencies
    libgomp1 \
    # ONNX Runtime library
    wget \
    # Cleanup
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Install ONNX Runtime
RUN wget https://github.com/microsoft/onnxruntime/releases/download/v1.16.0/onnxruntime-linux-x64-1.16.0.tgz \
    && tar -xzf onnxruntime-linux-x64-1.16.0.tgz \
    && cp onnxruntime-linux-x64-1.16.0/lib/libonnxruntime.so.1.16.0 /usr/lib/ \
    && rm -rf onnxruntime-linux-x64-1.16.0*

# Create non-root user for security
RUN groupadd -r hftbot -g 1000 && \
    useradd -r -u 1000 -g hftbot -m -s /bin/bash hftbot && \
    mkdir -p /app/logs /app/ml_training/models /app/migrations && \
    chown -R hftbot:hftbot /app

# Set working directory
WORKDIR /app

# Copy binary from builder stage
COPY --from=builder --chown=hftbot:hftbot /app/target/release/hft-arbitrage-bot /app/hft-arbitrage-bot
COPY --from=builder --chown=hftbot:hftbot /app/migrations /app/migrations

# Create directory structure for ML models and logs
VOLUME ["/app/logs", "/app/ml_training/models"]

# Switch to non-root user
USER hftbot

# Expose metrics and health check port
EXPOSE 8080

# Environment variables (can be overridden)
ENV RUST_LOG=info \
    METRICS_PORT=8080 \
    LOG_LEVEL=info \
    ENABLE_TRACING=true

# Health check configuration
HEALTHCHECK --interval=30s \
            --timeout=10s \
            --start-period=40s \
            --retries=3 \
    CMD curl -f http://localhost:${METRICS_PORT}/health || exit 1

# Run the application
ENTRYPOINT ["/app/hft-arbitrage-bot"]
