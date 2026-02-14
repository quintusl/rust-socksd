# AGENTS.md

This file provides guidance for AI agents when working with code in this repository.

## Project Overview

rust-socksd is a high-performance SOCKS5 and HTTP proxy server written in Rust with async architecture. The codebase features:

- **Dual Protocol Support**: SOCKS5 and HTTP proxy protocols running on separate ports
- **Async Architecture**: Built on tokio with concurrent connection handling 
- **Modular Authentication**: Support for File-based, PAM, LDAP, and Database (MySQL/PostgreSQL) backends
- **Configuration Management**: YAML-based main config with modular backend setup
- **Security Features**: Network restrictions, domain blocking, rate limiting
- **Multi-level CLI**: Main server + user management + validation subcommands

## Core Architecture

### Main Components

- `src/main.rs` - CLI argument parsing, logging setup, and application entry point
- `src/server.rs` - `ProxyServer` orchestrates SOCKS5/HTTP listeners and initializes the `Authenticator`
- `src/socks5.rs` - SOCKS5 protocol implementation using `Authenticator` trait
- `src/http_proxy.rs` - HTTP proxy handler with `Authenticator` support (Basic Auth)
- `src/auth/` - Modular authentication system:
  - `mod.rs`: `Authenticator` trait definition
  - `simple.rs`: File-based auth (Argon2/Bcrypt/Scrypt)
  - `pam.rs`: PAM integration (feature-gated)
  - `ldap.rs`: LDAP integration
  - `sql.rs`: Database integration (sqlx)
- `src/config.rs` - Configuration structs including `AuthBackendConfig` enum
- `src/lib.rs` - Public API exports

### Configuration Architecture

The project uses a structured configuration approach:
- **Main config** (`config.yml`) - Server settings, auth backend selection, logging, security
- **Auth Backends**:
  - `simple`: Uses independent `users.yml` file
  - `pam`: Uses system PAM service
  - `ldap`: Uses LDAP directory
  - `database`: Uses SQL connection string
- Config validation ensures required fields for selected backend are present

### Connection Flow

1. `ProxyServer::create()` initializes the configured `Authenticator` (async)
2. `ProxyServer::start()` spawns SOCKS5 and HTTP listeners
3. Each connection acquires a semaphore permit
4. **SOCKS5**: Handshake -> `Authenticator::authenticate()` -> Request -> Tunnel
5. **HTTP**: Request parsing -> `Authenticator::authenticate()` (Basic Auth) -> CONNECT/Proxy
6. Data relay using `tokio::io::copy_bidirectional`

## Development Commands

### Building and Running
```bash
# Build release binary (includes PAM by default)
cargo build --release

# Build without PAM (useful for macOS/cross-compilation issues)
cargo build --release --no-default-features

# Run with custom config
cargo run -- --config config.yml
```

### Testing
```bash
# Run unit tests
cargo test
```

# Test with tokio-test features
cargo test --features tokio-test
```

### User Management
```bash
# Initialize user config
cargo run -- user init --user-config users.yml --hash-type argon2

# Add user (will prompt for password)
cargo run -- user add --user-config users.yml username

# List users
cargo run -- user list --user-config users.yml
```

### Validation
```bash
# Validate both config files
cargo run -- validate --config config.yml --user-config users.yml
```

### Package Building
```bash
# Debian package
dpkg-buildpackage -b

# Check systemd service
systemctl --user status rust-socksd
```

## Key Implementation Notes

- Connection semaphore limits concurrent connections globally across both protocols
- Authentication is only applied to SOCKS5 (HTTP proxy runs without auth)
- User config uses secure password hashing with salt generation
- All async operations use proper timeout handling
- Error responses follow protocol-specific format (SOCKS5 response codes vs HTTP status)
- Data relay uses tokio::io::copy for zero-copy performance
- Logging integrates console, file, and journald outputs
- Configuration validation ensures port conflicts and network address formats
