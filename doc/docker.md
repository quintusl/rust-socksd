# Docker & Containerization

`rust-socksd` includes support for containerized deployment with multi-stage builds.

---

## Quick Start

### Using the Pre-built Image

Run the server out-of-the-box using the latest image:

```bash
docker run -d \
  --name rust-socksd \
  -p 1080:1080 \
  -p 8080:8080 \
  quintux/rust-socksd:latest
```

### Building From Source

You can build the Docker image locally from the project root:

```bash
# Build the Docker image
docker build -t rust-socksd .

# Run the container
docker run -d \
  --name rust-socksd \
  -p 1080:1080 \
  -p 8080:8080 \
  rust-socksd
```

---

## Configuration in Docker

### 1. Using Environment Variables

For simple configurations, pass environment variables directly:

```bash
docker run -d \
  --name rust-socksd \
  -p 1080:1080 \
  -p 8080:8080 \
  -e RUST_SOCKSD_BIND_ADDRESS="0.0.0.0" \
  -e RUST_SOCKSD_SOCKS5_PORT="1080" \
  -e RUST_SOCKSD_HTTP_PORT="8080" \
  -e RUST_SOCKSD_LOG_LEVEL="info" \
  quintux/rust-socksd:latest
```

### 2. Using Custom Config Files

Mount a host directory containing configurations to load custom settings dynamically:

```bash
# Create local configuration directory
mkdir -p ./config

# Generate default configuration inside the mounted directory
docker run --rm -v ./config:/config quintux/rust-socksd:latest --generate-config /config/config.yml

# Edit the config.yml as needed (e.g. enabling auth or egress rules)
# Then start the container referencing the configuration file
docker run -d \
  --name rust-socksd \
  -p 1080:1080 \
  -p 8080:8080 \
  -v ./config:/config \
  quintux/rust-socksd:latest --config /config/config.yml
```

---

## Docker Compose

Save the following content to a `docker-compose.yml` file to orchestrate the service:

```yaml
version: '3.8'

services:
  rust-socksd:
    image: quintux/rust-socksd:latest
    container_name: rust-socksd
    ports:
      - "1080:1080"  # SOCKS5 port
      - "8080:8080"  # HTTP proxy port
      - "8081:8081"  # Admin port (Optional, expose only if needed)
    environment:
      - RUST_SOCKSD_BIND_ADDRESS=0.0.0.0
      - RUST_SOCKSD_LOG_LEVEL=info
    volumes:
      - ./config:/config  # Mount config directory
    restart: unless-stopped
    security_opt:
      - no-new-privileges:true
    user: "1001:1001"  # Run as non-root user
```

---

## Image Security Hardening

The official `rust-socksd` image utilizes container security best practices:

* **Multi-stage Build**: Eliminates build-time dependencies, compiler toolchains, and source code, resulting in a minimal runtime package.
* **Non-root Execution**: Runs as an isolated unprivileged user `appuser` (UID `1001` / GID `1001`).
* **Minimal Base OS**: Uses `debian:bullseye-slim` for a minimal attack surface.
* **No Privilege Escalation**: Configured to deny requests for new privileges (`no-new-privileges:true`).
