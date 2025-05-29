# Stage 1: Build the application
# Use a specific Rust version for reproducibility, slim variant for smaller base
FROM rust:1.87-slim-bullseye AS builder
LABEL maintainer="Gemini Code Assist"

# Install system dependencies if needed by any crates (e.g., libssl-dev, pkg-config for TLS/crypto)
# RUN apt-get update && apt-get install -y libssl-dev pkg-config

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Download dependencies.
# This layer is cached and re-run only if Cargo.toml or Cargo.lock change.
RUN cargo fetch

# Copy source code
# This layer is cached and re-run only if files in src/ change.
COPY src ./src

# Build the application in release mode
# This step uses downloaded/cached dependencies.
RUN cargo build --release --locked

# Stage 2: Create the final lightweight image
FROM debian:bullseye-slim AS runner

# Arguments for user/group creation, can be overridden at build time
ARG UID=1001
ARG GID=1001

# Create a non-root user and group for security
RUN groupadd -g ${GID} appgroup && \
    useradd -u ${UID} -g appgroup -ms /bin/bash -d /app appuser

WORKDIR /app

# Copy the compiled binary from the builder stage to a common bin location
COPY --from=builder /app/target/release/rusty-socks /usr/local/bin/rusty-socks

# Ensure the binary is executable
RUN chmod +x /usr/local/bin/rusty-socks

# Expose the default SOCKS5 port
EXPOSE 1080
# Consider exposing the HTTP port if it's commonly used and has a fixed default
# EXPOSE 8080

# Switch to the non-root user
USER appuser

# Set default environment variables for rusty-socks configuration.
# Assumes rusty-socks reads these.

# Listen on all interfaces inside the container
ENV RUSTY_SOCKS_BIND_ADDRESS="0.0.0.0"
# Default SOCKS5 port
ENV RUSTY_SOCKS_SOCKS5_PORT="1080"
# Default HTTP port (adjust if needed)
ENV RUSTY_SOCKS_HTTP_PORT="8080"
# Log level
ENV RUSTY_SOCKS_LOG_LEVEL="info"

# Default command to run the application
CMD ["rusty-socks"]