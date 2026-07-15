# CLI & Usage Guide

This guide details the command-line options available for `rust-socksd` and instructions on configuring proxy client applications.

---

## Command Line Interface (CLI)

### Main Server Entry

Run `rust-socksd` using standard flags to start the daemon.

```bash
rust-socksd [OPTIONS] [SUBCOMMAND]
```

#### Global Options

* **`-c, --config <FILE>`**: Path to the YAML configuration file (default: `config.yml`).
* **`-g, --generate-config <FILE>`**: Generates a default YAML configuration file and exits (conflicts with `--config`).
* **`-v, --verbose`**: Enables verbose logging (can be specified multiple times for deeper trace levels).
* **`-q, --quiet`**: Suppresses all standard output logs except errors.
* **`-b, --bind <ADDRESS>`**: Binds the proxy listeners to the specified IP address.
* **`-p, --http-port <PORT>`**: Port for the HTTP proxy.
* **`-s, --socks5-port <PORT>`**: Port for the SOCKS5 proxy.
* **`-l, --loglevel <LEVEL>`**: Logging filter level: `trace`, `debug`, `info`, `warn`, `error`.
* **`--admin-port <PORT>`**: Port for the administrative API listener.
* **`--admin-enabled`**: Enables the admin server listener at boot time.
* **`-h, --help`**: Prints CLI help and usage details.
* **`-V, --version`**: Prints application version information.

#### Subcommands
* **`validate`**: Validates the syntax of configuration files.
* **`user`**: Manages credentials database for Simple authentication backend.

---

### Configuration Validation Subcommand

Validate your main configuration or user configuration syntax before launching the server:

```bash
rust-socksd validate [OPTIONS]
```

#### Subcommand Options
* **`-c, --config <FILE>`**: Main config file to validate (default: `config.yml`).
* **`--user-config <FILE>`**: Simple backend user file to validate (optional).

#### Examples
```bash
# Validate default config.yml
rust-socksd validate

# Validate specific configurations
rust-socksd validate --config /etc/socksd/config.yml --user-config /etc/socksd/users.yml
```

---

### User Management Subcommand

Manages user credentials for the `simple` authenticator backend file. See [Authentication Backends Guide](authentication.md) for details.

```bash
rust-socksd user [OPTIONS] <SUBCOMMAND> [SUBCOMMAND_ARGS]
```

---

## Client Proxy Usage

Once `rust-socksd` is running, configure client applications to route traffic through it.

### SOCKS5 Client Setup

The SOCKS5 proxy implements the standard SOCKS5 protocol (RFC 1928), supporting username/password auth (RFC 1929).

* **Protocol**: SOCKS5 (or SOCKS5h to resolve DNS names remotely on the proxy side)
* **Default Port**: `1080`

#### curl Example (Unauthenticated)
```bash
curl --socks5-hostname localhost:1080 https://httpbin.org/ip
```

#### curl Example (With Credentials)
```bash
curl --socks5-hostname localhost:1080 --user "username:password" https://httpbin.org/ip
```

---

### HTTP Client Setup

The HTTP proxy supports HTTP/HTTPS tunnel connections via the `CONNECT` method and regular proxy requests, optionally requiring Basic Authentication.

* **Protocol**: HTTP / HTTPS
* **Default Port**: `8080`

#### curl Example (Unauthenticated)
```bash
curl --proxy localhost:8080 https://httpbin.org/ip
```

#### curl Example (With Credentials)
```bash
curl --proxy localhost:8080 --proxy-user "username:password" https://httpbin.org/ip
```
