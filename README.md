# rust-socksd

A high-performance SOCKS5 and HTTP proxy server written in Rust, featuring modern async architecture, modular authentication, and comprehensive security features.

---

## Features

* **Dual Protocol Support**: Complete implementation of SOCKS5 (RFC 1928) and HTTP/HTTPS (CONNECT method) proxies running on separate ports.
* **Authentication**: Seamless username/password verification (Basic Auth for HTTP) using modular backends (Simple file-based, Linux PAM, LDAP, or SQL database).
* **Upstream Chaining**: Chain outgoing connections through upstream SOCKS5 or HTTP proxies, including bypass rules and environment variable support.
* **Granular Security Controls**: Source network restrictions (ingress ACLs), destination filters (egress ACLs), domain blocking, rate limiting, and request size controls.
* **Admin HTTP API**: Dedicated endpoint for real-time metrics, dynamic configuration validation, liveness/health probing, and hot reloads.
* **Production Hardened**: Native systemd service integration, Debian/Ubuntu packaging support, Arch Linux AUR, and multi-stage secure Docker containers.

---

## Quick Start

### Build and Run from Source

```bash
# Clone the repository
git clone https://github.com/quintusl/rust-socksd.git
cd rust-socksd

# Build in release mode
cargo build --release

# Generate a default configuration file
./target/release/rust-socksd --generate-config config.yml

# Start the proxy daemon
./target/release/rust-socksd --config config.yml
```

For package installations, Docker configurations, and platform-specific guides, see the [Quick Start Guide](doc/quickstart.md).

---

## Documentation Map

Detailed guides are split into topic-specific files located in the `doc/` directory:

| Document | Description |
| :--- | :--- |
| **[Quick Start Guide](doc/quickstart.md)** | Installation steps for Source, Debian, Arch Linux, and Docker; verification checks. |
| **[Configuration Guide](doc/configuration.md)** | Explanation of settings, logging, security policies, upstream proxies, and environment variables. |
| **[Authentication Backends](doc/authentication.md)** | Setting up file-based user accounts, Linux PAM, LDAP directory, SQL database, and CLI user controls. |
| **[CLI & Usage Guide](doc/usage.md)** | Command-line arguments, options, the `validate` and `user` subcommands, and proxy client examples. |
| **[Docker & Containerization](doc/docker.md)** | Running with Docker and Docker Compose, secure environment settings, and runtime mounts. |
| **[Deployment & Packaging](doc/deployment.md)** | Setting up systemd services, reading journald logs, and building `.deb` or AUR packages. |
| **[Admin API Reference](doc/admin_api.md)** | Endpoints for Prometheus metrics, health checking, config inspections, schema validation, and hot reload. |
| **[Troubleshooting Guide](doc/troubleshooting.md)** | Common errors (ports, permissions, connection, auth) and log inspecting techniques. |

---

## Performance

`rust-socksd` is optimized for low-latency and high-throughput production usage:

* **Async Architecture**: Built on the Tokio runtime for massive concurrent socket handling.
* **Zero-copy Relay**: Employs optimized I/O looping to minimize CPU overhead and buffer copying.
* **Minimal Footprint**: Operates with a base memory footprint under ~10MB under idle loads.
* **Scalable limits**: Supports over 1000+ simultaneous connections out-of-the-box.

---

## Build Features

The project supports optional compilation features:

* **`pam-auth`** (default): Enables Pluggable Authentication Modules support (requires `libpam` development headers).
  * *Note for macOS*: This feature requires LLVM compiler headers (`brew install llvm`) for bindgen. If you experience build errors, build with `--no-default-features` to compile without PAM.

To compile without PAM support:
```bash
cargo build --release --no-default-features
```

---

## License

This project is licensed under either of:
* MIT License ([LICENSE-MIT](LICENSE.MIT))
* Apache License, Version 2.0 ([LICENSE-Apache-2.0](LICENSE.Apache-2.0))

at your option.

`SPDX-License-Identifier: MIT OR Apache-2.0`
