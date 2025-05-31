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

#### Docker
```bash
# Pull and run the latest Docker image
docker run -d \
  --name rusty-socks \
  -p 1080:1080 \
  -p 8080:8080 \
  ghcr.io/quintusl/rusty-socks:latest
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

Rusty SOCKS supports secure username/password authentication using separate configuration files for enhanced security. Users are managed through dedicated CLI commands with secure password hashing.

#### Enabling Authentication

To enable authentication in your main configuration:

```yaml
auth:
  enabled: true
  method: "username_password"
  user_config_file: "users.yml"
```

#### User Management

Users are managed separately from the main configuration using the `user` subcommand:

##### Initialize User Configuration
```bash
# Create a new user configuration file
rusty-socks user init --user-config users.yml --hash-type argon2
```

##### Add Users
```bash
# Add a new user (will prompt for password)
rusty-socks user add --user-config users.yml username

# Add user with password specified
rusty-socks user add --user-config users.yml username password

# Specify hash type (argon2, bcrypt, scrypt)
rusty-socks user add --user-config users.yml --hash-type bcrypt username
```

##### Manage Existing Users
```bash
# List all users
rusty-socks user list --user-config users.yml

# Update user password
rusty-socks user update --user-config users.yml username

# Enable/disable a user
rusty-socks user enable --user-config users.yml username true
rusty-socks user enable --user-config users.yml username false

# Remove a user
rusty-socks user remove --user-config users.yml username
```

#### Password Security

Rusty SOCKS uses secure password hashing with support for:
- **Argon2** (default, recommended)
- **bcrypt** 
- **scrypt**

Passwords are never stored in plain text and use cryptographically secure salt generation.

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

### Main Command

```
rusty-socks [OPTIONS] [SUBCOMMAND]

OPTIONS:
    -c, --config <FILE>              Configuration file path [default: config.yml]
    -g, --generate-config <FILE>     Generate a default configuration file
    -v, --verbose                    Enable verbose logging (can be used multiple times)
    -q, --quiet                      Suppress all output except errors
    -b, --bind <ADDRESS>             Bind address (can also be set via RUSTY_SOCKS_BIND_ADDRESS)
    -p, --http-port <PORT>           HTTP proxy port (can also be set via RUSTY_SOCKS_HTTP_PORT)
    -s, --socks5-port <PORT>         SOCKS5 proxy port (can also be set via RUSTY_SOCKS_SOCKS5_PORT)
    -l, --loglevel <LEVEL>           Log level: trace, debug, info, warn, error (can also be set via RUSTY_SOCKS_LOG_LEVEL)
    -h, --help                       Print help information
    -V, --version                    Print version information

SUBCOMMANDS:
    validate                         Validate configuration files
    user                            User management commands
```

### Validation Subcommand

Validate configuration files for syntax and consistency:

```
rusty-socks validate [OPTIONS]

OPTIONS:
    -c, --config <FILE>              Configuration file to validate [default: config.yml]
        --user-config <FILE>         User configuration file to validate
```

Examples:
```bash
# Validate main configuration
rusty-socks validate

# Validate specific config files
rusty-socks validate --config /etc/rusty-socks/config.yml --user-config /etc/rusty-socks/users.yml
```

### User Management Subcommand

Manage user accounts with secure password hashing:

```
rusty-socks user [OPTIONS] <SUBCOMMAND>

OPTIONS:
        --user-config <FILE>         User configuration file path [default: users.yml]

SUBCOMMANDS:
    init                            Initialize a new user configuration file
    add                             Add a new user
    remove                          Remove a user
    list                            List all users
    update                          Update user password
    enable                          Enable/disable a user
```

#### User Subcommand Details

##### Initialize User Config
```
rusty-socks user init [OPTIONS]

OPTIONS:
        --hash-type <TYPE>           Default password hash type: argon2, bcrypt, scrypt [default: argon2]
        --user-config <FILE>         User configuration file path [default: users.yml]
```

##### Add User
```
rusty-socks user add [OPTIONS] <USERNAME> [PASSWORD]

ARGUMENTS:
    <USERNAME>                       Username
    [PASSWORD]                       Password (will prompt if not provided)

OPTIONS:
        --hash-type <TYPE>           Password hash type: argon2, bcrypt, scrypt [default: argon2]
        --user-config <FILE>         User configuration file path [default: users.yml]
```

##### Remove User
```
rusty-socks user remove [OPTIONS] <USERNAME>

ARGUMENTS:
    <USERNAME>                       Username to remove

OPTIONS:
        --user-config <FILE>         User configuration file path [default: users.yml]
```

##### List Users
```
rusty-socks user list [OPTIONS]

OPTIONS:
        --user-config <FILE>         User configuration file path [default: users.yml]
```

##### Update User Password
```
rusty-socks user update [OPTIONS] <USERNAME> [PASSWORD]

ARGUMENTS:
    <USERNAME>                       Username
    [PASSWORD]                       New password (will prompt if not provided)

OPTIONS:
        --user-config <FILE>         User configuration file path [default: users.yml]
```

##### Enable/Disable User
```
rusty-socks user enable [OPTIONS] <USERNAME> <ENABLED>

ARGUMENTS:
    <USERNAME>                       Username
    <ENABLED>                        Enable (true) or disable (false)

OPTIONS:
        --user-config <FILE>         User configuration file path [default: users.yml]
```

## Environment Variables

Rusty SOCKS supports several environment variables for configuration override:

- **RUSTY_SOCKS_BIND_ADDRESS**: Override the bind address (e.g., `0.0.0.0`)
- **RUSTY_SOCKS_SOCKS5_PORT**: Override the SOCKS5 port (e.g., `1080`)
- **RUSTY_SOCKS_HTTP_PORT**: Override the HTTP proxy port (e.g., `8080`)
- **RUSTY_SOCKS_LOG_LEVEL**: Override the log level (`trace`, `debug`, `info`, `warn`, `error`)

These environment variables take precedence over configuration file settings but are overridden by command line arguments.

### Examples

```bash
# Start with custom bind address and ports
export RUSTY_SOCKS_BIND_ADDRESS="0.0.0.0"
export RUSTY_SOCKS_SOCKS5_PORT="1081"
export RUSTY_SOCKS_HTTP_PORT="8081"
rusty-socks --config config.yml

# Start with debug logging
export RUSTY_SOCKS_LOG_LEVEL="debug"
rusty-socks --config config.yml

# Override specific settings via command line (takes highest precedence)
RUSTY_SOCKS_BIND_ADDRESS="0.0.0.0" rusty-socks --socks5-port 1082 --config config.yml
```

## Docker Support

Rusty SOCKS provides official Docker support with multi-stage builds for optimal security and performance.

### Quick Start with Docker

#### Using Pre-built Image

```bash
# Pull and run the latest image
docker run -d \
  --name rusty-socks \
  -p 1080:1080 \
  -p 8080:8080 \
  ghcr.io/quintusl/rusty-socks:latest
```

#### Building from Source

```bash
# Build the Docker image
docker build -t rusty-socks .

# Run the container
docker run -d \
  --name rusty-socks \
  -p 1080:1080 \
  -p 8080:8080 \
  rusty-socks
```

### Docker Configuration

#### Using Environment Variables

```bash
docker run -d \
  --name rusty-socks \
  -p 1080:1080 \
  -p 8080:8080 \
  -e RUSTY_SOCKS_BIND_ADDRESS="0.0.0.0" \
  -e RUSTY_SOCKS_SOCKS5_PORT="1080" \
  -e RUSTY_SOCKS_HTTP_PORT="8080" \
  -e RUSTY_SOCKS_LOG_LEVEL="info" \
  rusty-socks
```

#### Using Custom Configuration Files

```bash
# Create a directory for configuration files
mkdir -p ./config

# Generate default configuration
docker run --rm -v ./config:/config rusty-socks --generate-config /config/config.yml

# Edit the configuration file
nano ./config/config.yml

# Run with custom configuration
docker run -d \
  --name rusty-socks \
  -p 1080:1080 \
  -p 8080:8080 \
  -v ./config:/config \
  rusty-socks --config /config/config.yml
```

#### Docker Compose

```yaml
version: '3.8'

services:
  rusty-socks:
    image: ghcr.io/quintusl/rusty-socks:latest
    container_name: rusty-socks
    ports:
      - "1080:1080"  # SOCKS5 port
      - "8080:8080"  # HTTP proxy port
    environment:
      - RUSTY_SOCKS_BIND_ADDRESS=0.0.0.0
      - RUSTY_SOCKS_LOG_LEVEL=info
    volumes:
      - ./config:/config  # Optional: mount config directory
    restart: unless-stopped
    security_opt:
      - no-new-privileges:true
    user: "1001:1001"  # Run as non-root user
```

### Docker Security Features

The Docker image includes several security enhancements:

- **Multi-stage Build**: Minimal runtime image with only necessary components
- **Non-root User**: Runs as user `appuser` (UID 1001) for security
- **Minimal Base**: Uses `debian:bullseye-slim` for reduced attack surface
- **No Privileges**: Container runs without additional privileges
- **Exposed Ports**: Only necessary ports (1080, 8080) are exposed

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
