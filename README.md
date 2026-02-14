# rust-socksd

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
git clone https://github.com/quintusl/rust-socksd.git
cd rust-socksd
cargo build --release
sudo cp target/release/rust-socksd /usr/local/bin/
```

#### Debian/Ubuntu

```bash
sudo dpkg -i rust-socksd_0.1.1-1_amd64.deb
```

#### Arch Linux

```bash
yay -S rust-socksd
```

#### Docker

```bash
# Pull and run the latest Docker image
docker run -d \
  --name rust-socksd \
  -p 1080:1080 \
  -p 8080:8080 \
  quintux/rust-socksd:latest
```

### Running

#### Direct execution

```bash
rust-socksd --config config.yml
```

#### As a systemd service

```bash
sudo systemctl enable rust-socksd
sudo systemctl start rust-socksd
```

## Configuration

The server is configured using a YAML file. See `config.yml.example` for a complete example with all options.

Generate a default configuration file:

```bash
rust-socksd --generate-config config.yml
```

Edit the configuration file to suit your needs:

```bash
nano config.yml
```

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
rust-socksd supports multiple authentication backends to secure your proxy.

#### Supported Backends

1. **Simple (File-based)**: Uses a local YAML file with secure password hashes (Argon2, Bcrypt, Scrypt).
2. **PAM**: Integrates with Linux Pluggable Authentication Modules (e.g., system users).
3. **LDAP**: Authenticates against an LDAP directory (e.g., Active Directory, OpenLDAP).
4. **Database**: Authenticates against a MySQL or PostgreSQL database.

#### Configuration Examples

##### 1. Simple (File-based)

```yaml
auth:
  enabled: true
  type: simple
  user_config_file: "config/users.yml"
```

Manage users via CLI:
```bash
rust-socksd user add --user-config config/users.yml myuser
```

##### 2. PAM

Authenticate using system users (requires running as root or with appropriate permissions):

```yaml
auth:
  enabled: true
  type: pam
  service: "socksd" # config file in /etc/pam.d/socksd
```

##### 3. LDAP

```yaml
auth:
  enabled: true
  type: ldap
  url: "ldap://ldap.example.com:389"
  base_dn: "ou=users,dc=example,dc=com"
  # Optional binding for search
  bind_dn: "cn=admin,dc=example,dc=com"
  bind_password: "admin_password"
  user_filter: "(uid={})" # {} is replaced by the username
```

##### 4. Database (MySQL/PostgreSQL)

Authenticate by fetching a password hash from a database. The hash is verified using the same secure algorithms as the Simple backend.

```yaml
auth:
  enabled: true
  type: database
  db_type: "mysql" # or "postgres"
  url: "mysql://socksd:password@localhost/socksd_db"
  query: "SELECT password_hash FROM users WHERE username = ?"
  hash_type: "argon2" # argon2, bcrypt, or scrypt
```

### Security configuration

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

```bash
rust-socksd [OPTIONS] [SUBCOMMAND]

OPTIONS:
    -c, --config <FILE>              Configuration file path [default: config.yml]
    -g, --generate-config <FILE>     Generate a default configuration file
    -v, --verbose                    Enable verbose logging (can be used multiple times)
    -q, --quiet                      Suppress all output except errors
    -b, --bind <ADDRESS>             Bind address (can also be set via RUST_SOCKSD_BIND_ADDRESS)
    -p, --http-port <PORT>           HTTP proxy port (can also be set via RUST_SOCKSD_HTTP_PORT)
    -s, --socks5-port <PORT>         SOCKS5 proxy port (can also be set via RUST_SOCKSD_SOCKS5_PORT)
    -l, --loglevel <LEVEL>           Log level: trace, debug, info, warn, error (can also be set via RUST_SOCKSD_LOG_LEVEL)
    -h, --help                       Print help information
    -V, --version                    Print version information

SUBCOMMANDS:
    validate                         Validate configuration files
    user                            User management commands
```

### Validation Subcommand

Validate configuration files for syntax and consistency:

```bash
rust-socksd validate [OPTIONS]

OPTIONS:
    -c, --config <FILE>              Configuration file to validate [default: config.yml]
        --user-config <FILE>         User configuration file to validate
```

Examples:

```bash
# Validate main configuration
rust-socksd validate

# Validate specific config files
rust-socksd validate --config /etc/rust-socksd/config.yml --user-config /etc/rust-socksd/users.yml
```

### User Management Subcommand

Manage user accounts with secure password hashing:

```bash
rust-socksd user [OPTIONS] <SUBCOMMAND>

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

```bash
rust-socksd user init [OPTIONS]

OPTIONS:
        --hash-type <TYPE>           Default password hash type: argon2, bcrypt, scrypt [default: argon2]
        --user-config <FILE>         User configuration file path [default: users.yml]
```

##### Add User

```bash
rust-socksd user add [OPTIONS] <USERNAME> [PASSWORD]

ARGUMENTS:
    <USERNAME>                       Username
    [PASSWORD]                       Password (will prompt if not provided)

OPTIONS:
        --hash-type <TYPE>           Password hash type: argon2, bcrypt, scrypt [default: argon2]
        --user-config <FILE>         User configuration file path [default: users.yml]
```

##### Remove User

```bash
rust-socksd user remove [OPTIONS] <USERNAME>

ARGUMENTS:
    <USERNAME>                       Username to remove

OPTIONS:
        --user-config <FILE>         User configuration file path [default: users.yml]
```

##### List Users

```bash
rust-socksd user list [OPTIONS]

OPTIONS:
        --user-config <FILE>         User configuration file path [default: users.yml]
```

##### Update User Password

```bash
rust-socksd user update [OPTIONS] <USERNAME> [PASSWORD]

ARGUMENTS:
    <USERNAME>                       Username
    [PASSWORD]                       New password (will prompt if not provided)

OPTIONS:
        --user-config <FILE>         User configuration file path [default: users.yml]
```

##### Enable/Disable User

```bash
rust-socksd user enable [OPTIONS] <USERNAME> <ENABLED>

ARGUMENTS:
    <USERNAME>                       Username
    <ENABLED>                        Enable (true) or disable (false)

OPTIONS:
        --user-config <FILE>         User configuration file path [default: users.yml]
```

## Environment Variables

rust-socksd supports several environment variables for configuration override:

- **RUST_SOCKSD_BIND_ADDRESS**: Override the bind address (e.g., `0.0.0.0`)
- **RUST_SOCKSD_SOCKS5_PORT**: Override the SOCKS5 port (e.g., `1080`)
- **RUST_SOCKSD_HTTP_PORT**: Override the HTTP proxy port (e.g., `8080`)
- **RUST_SOCKSD_LOG_LEVEL**: Override the log level (`trace`, `debug`, `info`, `warn`, `error`)

These environment variables take precedence over configuration file settings but are overridden by command line arguments.

### Examples

```bash
# Start with custom bind address and ports
export RUST_SOCKSD_BIND_ADDRESS="0.0.0.0"
export RUST_SOCKSD_SOCKS5_PORT="1081"
export RUST_SOCKSD_HTTP_PORT="8081"
rust-socksd --config config.yml

# Start with debug logging
export RUST_SOCKSD_LOG_LEVEL="debug"
rust-socksd --config config.yml

# Override specific settings via command line (takes highest precedence)
RUST_SOCKSD_BIND_ADDRESS="0.0.0.0" rust-socksd --socks5-port 1082 --config config.yml
```

## Docker Support

rust-socksd provides official Docker support with multi-stage builds for optimal security and performance.

### Quick Start with Docker

#### Using Pre-built Image

```bash
# Pull and run the latest image
docker run -d \
  --name rust-socksd \
  -p 1080:1080 \
  -p 8080:8080 \
  quintux/rust-socksd:latest
```

#### Building from Source

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

### Docker Configuration

#### Using Environment Variables

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

#### Using Custom Configuration Files

```bash
# Create a directory for configuration files
mkdir -p ./config

# Generate default configuration
docker run --rm -v ./config:/config quintux/rust-socksd:latest --generate-config /config/config.yml

# Edit the configuration file
nano ./config/config.yml

# Run with custom configuration
docker run -d \
  --name rust-socksd \
  -p 1080:1080 \
  -p 8080:8080 \
  -v ./config:/config \
  quintux/rust-socksd:latest --config /config/config.yml
```

#### Docker Compose

```yaml
version: '3.8'

services:
  rust-socksd:
    image: quintux/rust-socksd:latest
    container_name: rust-socksd
    ports:
      - "1080:1080"  # SOCKS5 port
      - "8080:8080"  # HTTP proxy port
    environment:
      - RUST_SOCKSD_BIND_ADDRESS=0.0.0.0
      - RUST_SOCKSD_LOG_LEVEL=info
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
- Proper user isolation (runs as `rust-socksd` user)
- Security hardening (restricted filesystem access, no new privileges)
- Service restart on failure
- Proper logging integration

Service management:

```bash
# Start the service
sudo systemctl start rust-socksd

# Enable automatic startup
sudo systemctl enable rust-socksd

# Check status
sudo systemctl status rust-socksd

# View logs
sudo journalctl -u rust-socksd -f
```

## Building Packages

### Debian Package

```bash
# Install build dependencies
sudo apt-get install debhelper-compat cargo rustc

# Build the package
dpkg-buildpackage -b -uc -us
```

### Arch Package

```bash
# Build from AUR
git clone https://aur.archlinux.org/rust-socksd.git
cd rust-socksd
makepkg -si
```

## Performance

rust-socksd is designed for high performance:

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
rust-socksd --config config.yml --verbose
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
sudo journalctl -u rust-socksd

# Application logs (if file logging enabled)
sudo tail -f /var/log/rust-socksd/rust-socksd.log
```

## License

This project is licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

### Build Features

The project supports the following Cargo features:

- **pam-auth** (default): Enables PAM authentication backend. Requires `libpam` development headers.
  - *Note for macOS*: This feature may require `llvm` to be installed (`brew install llvm`) for `bindgen`. If you encounter build errors, you can disable this feature.

To build without PAM support:
```bash
cargo build --release --no-default-features
```

Contributions are welcome! Please feel free to submit a Pull Request.

## Support

For issues, questions, or feature requests, please visit our [GitHub Issues](https://github.com/quintusl/rust-socksd/issues) page.
