# Rusty SOCKS

A high-performance SOCKS5 and HTTP proxy server written in Rust, featuring modern async architecture and comprehensive security features.

## Features

- **Full SOCKS5 Protocol Support**: Complete implementation with authentication
- **HTTP Proxy**: Support for HTTP/HTTPS with CONNECT method
- **Multi-threaded Architecture**: Built on tokio for high concurrency
- **YAML Configuration**: Flexible and easy-to-use configuration system
- **Authentication Support**: Username/password authentication for SOCKS5
- **Security Features**: Network restrictions, domain blocking, rate limiting
- **Systemd Integration**: Native Linux service support
- **Comprehensive Logging**: Configurable logging with multiple levels
- **Package Support**: Debian packages and Arch AUR available

## Quick Start

### Installation

#### From Source
```bash
git clone https://github.com/quintusl/rusty-socks.git
cd rusty-socks
cargo build --release
sudo cp target/release/rusty-socks /usr/local/bin/
```

#### Debian/Ubuntu
```bash
sudo dpkg -i rusty-socks_0.1.0-1_amd64.deb
```

#### Arch Linux
```bash
yay -S rusty-socks
```

### Configuration

Generate a default configuration file:
```bash
rusty-socks --generate-config config.yml
```

Edit the configuration file to suit your needs:
```bash
nano config.yml
```

### Running

#### Direct execution:
```bash
rusty-socks --config config.yml
```

#### As a systemd service:
```bash
sudo systemctl enable rusty-socks
sudo systemctl start rusty-socks
```

## Configuration

The server is configured using a YAML file. See `config.yml.example` for a complete example with all options.

### Basic Configuration

```yaml
server:
  bind_address: "127.0.0.1"
  socks5_port: 1080
  http_port: 8080
  max_connections: 1000
  connection_timeout: 300

auth:
  enabled: false
  method: "none"

logging:
  level: "info"
  console: true
```

### Authentication

To enable authentication:

```yaml
auth:
  enabled: true
  method: "username_password"
  users:
    - username: "user1"
      password: "password1"
    - username: "admin"
      password: "secure_password"
```

### Security

Configure network restrictions and domain blocking:

```yaml
security:
  allowed_networks:
    - "192.168.1.0/24"
    - "10.0.0.0/8"
  blocked_domains:
    - "malicious-site.com"
    - "blocked-domain.net"
  rate_limit:
    requests_per_minute: 1000
    burst_size: 100
```

## Usage

### SOCKS5 Proxy

Configure your applications to use the SOCKS5 proxy:
- **Host**: Your server IP
- **Port**: 1080 (default)
- **Authentication**: Username/password (if enabled)

Example with curl:
```bash
curl --socks5-hostname localhost:1080 https://httpbin.org/ip
```

### HTTP Proxy

Configure your applications to use the HTTP proxy:
- **Host**: Your server IP
- **Port**: 8080 (default)

Example with curl:
```bash
curl --proxy localhost:8080 https://httpbin.org/ip
```

## Command Line Options

```
rusty-socks [OPTIONS]

OPTIONS:
    -c, --config <FILE>              Configuration file path [default: config.yml]
    -g, --generate-config <FILE>     Generate a default configuration file
    -v, --verbose                    Enable verbose logging (can be used multiple times)
    -q, --quiet                      Suppress all output except errors
    -h, --help                       Print help information
    -V, --version                    Print version information
```

## Systemd Service

The package includes a systemd service file that provides:

- Automatic startup on boot
- Proper user isolation (runs as `rusty-socks` user)
- Security hardening (restricted filesystem access, no new privileges)
- Service restart on failure
- Proper logging integration

Service management:
```bash
# Start the service
sudo systemctl start rusty-socks

# Enable automatic startup
sudo systemctl enable rusty-socks

# Check status
sudo systemctl status rusty-socks

# View logs
sudo journalctl -u rusty-socks -f
```

## Building Packages

### Debian Package

```bash
# Install build dependencies
sudo apt-get install debhelper-compat cargo rustc

# Build the package
dpkg-buildpackage -b
```

### Arch Package

```bash
# Build from AUR
git clone https://aur.archlinux.org/rusty-socks.git
cd rusty-socks
makepkg -si
```

## Performance

Rusty SOCKS is designed for high performance:

- **Async Architecture**: Built on tokio for efficient I/O handling
- **Zero-copy Operations**: Minimal memory allocations during data transfer
- **Connection Pooling**: Efficient connection management
- **Configurable Limits**: Tunable for your specific use case

Typical performance on modern hardware:
- **Throughput**: 1000+ concurrent connections
- **Latency**: Sub-millisecond proxy overhead
- **Memory**: ~10MB base memory usage

## Security

Security features include:

- **Network Restrictions**: Allow/deny lists for source networks
- **Domain Blocking**: Block access to specific domains
- **Authentication**: Secure username/password authentication
- **Rate Limiting**: Prevent abuse and DoS attacks
- **Service Isolation**: Runs with minimal privileges
- **Secure Defaults**: Conservative default configuration

## Troubleshooting

### Common Issues

1. **Permission Denied**: Ensure the service user has access to configuration files
2. **Port Already in Use**: Check if another service is using the same ports
3. **Connection Refused**: Verify firewall settings and bind address
4. **Authentication Failures**: Check username/password configuration

### Debugging

Enable debug logging:
```bash
rusty-socks --config config.yml --verbose
```

Or set in configuration:
```yaml
logging:
  level: "debug"
```

### Logs

Check system logs:
```bash
# Systemd service logs
sudo journalctl -u rusty-socks

# Application logs (if file logging enabled)
sudo tail -f /var/log/rusty-socks/rusty-socks.log
```

## License

This project is licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Support

For issues, questions, or feature requests, please visit our [GitHub Issues](https://github.com/quintusl/rusty-socks/issues) page.
