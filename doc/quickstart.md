# Quick Start Guide

This guide will help you install and run `rust-socksd` on various platforms.

## Installation

### From Source

Ensure you have Rust and Cargo installed. Then run:

```bash
git clone https://github.com/quintusl/rust-socksd.git
cd rust-socksd
cargo build --release
sudo cp target/release/rust-socksd /usr/local/bin/
```

By default, this builds with PAM authentication support (which requires PAM development headers). If you are building on a platform without PAM development headers (like macOS) or want a minimal build, you can disable default features:

```bash
cargo build --release --no-default-features
```

### Debian/Ubuntu

You can install `rust-socksd` from a pre-built `.deb` package:

```bash
sudo dpkg -i rust-socksd_0.1.1-1_amd64.deb
```

### Arch Linux

Install from the AUR using your favorite helper (e.g., `yay`):

```bash
yay -S rust-socksd
```

### Docker

To pull and run the latest official image:

```bash
docker run -d \
  --name rust-socksd \
  -p 1080:1080 \
  -p 8080:8080 \
  quintux/rust-socksd:latest
```

---

## Running the Server

### Direct Execution

To run the server with a custom configuration file:

```bash
rust-socksd --config config.yml
```

### Systemd Service

If installed via package managers, you can manage the proxy server as a systemd service:

```bash
sudo systemctl enable rust-socksd
sudo systemctl start rust-socksd
```

For more deployment details, see [Deployment & Packaging Guide](deployment.md).

---

## Verifying Connections

You can verify that both the SOCKS5 and HTTP proxies are working by using `curl`.

### SOCKS5 Proxy
Assuming the SOCKS5 proxy is running on port `1080`:

```bash
curl --socks5-hostname localhost:1080 https://httpbin.org/ip
```

### HTTP Proxy
Assuming the HTTP proxy is running on port `8080`:

```bash
curl --proxy localhost:8080 https://httpbin.org/ip
```
