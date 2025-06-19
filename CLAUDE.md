# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

rust-socksd is a high-performance SOCKS5 and HTTP proxy server written in Rust with async architecture. The codebase features:

- **Dual Protocol Support**: SOCKS5 and HTTP proxy protocols running on separate ports
- **Async Architecture**: Built on tokio with concurrent connection handling 
- **Authentication System**: Separate user configuration with secure password hashing (Argon2, bcrypt, scrypt)
- **Configuration Management**: YAML-based main config with separate user management
- **Security Features**: Network restrictions, domain blocking, rate limiting
- **Multi-level CLI**: Main server + user management + validation subcommands

## Core Architecture

### Main Components

- `src/main.rs` - CLI argument parsing, logging setup, and application entry point
- `src/server.rs` - ProxyServer orchestrates both SOCKS5 and HTTP listeners with connection pooling
- `src/socks5.rs` - SOCKS5 protocol implementation with authentication support  
- `src/http_proxy.rs` - HTTP proxy handler for CONNECT and regular proxy requests
- `src/config/` - Configuration management split between main config and user config
- `src/lib.rs` - Public API exports

### Configuration Architecture

The project uses a split configuration approach:
- **Main config** (`config.yml`) - Server settings, auth config, logging, security rules
- **User config** (`users.yml`) - Separate file for user accounts with hashed passwords
- Both configs have validation and CLI management commands

### Connection Flow

1. `ProxyServer::start()` spawns two concurrent listeners (SOCKS5 + HTTP)
2. Each connection acquires a semaphore permit for connection limiting
3. Connections are handled with configurable timeouts
4. SOCKS5: handshake → authentication → request → tunnel establishment
5. HTTP: request parsing → CONNECT handling or regular proxy forwarding
6. Data relay using bidirectional tokio::io::copy for tunnel connections

## Development Commands

### Building and Running
```bash
# Build release binary
cargo build --release

# Run with custom config
cargo run -- --config config.yml

# Generate default config
cargo run -- --generate-config config.yml

# Enable debug logging
cargo run -- --config config.yml --verbose
```

### Testing
```bash
# Run unit tests
cargo test

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