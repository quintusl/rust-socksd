# Configuration Guide

The `rust-socksd` proxy server is configured using a YAML file. 

## Generation

You can generate a default configuration file with default values:

```bash
rust-socksd --generate-config config.yml
```

---

## Configuration Sections

A complete configuration file comprises the following main sections:

### 1. Basic Server Settings (`server`)

Controls ports, timeouts, limits, and buffer sizes.

```yaml
server:
  # Bind address for all listeners (default: 127.0.0.1)
  bind_address: "127.0.0.1"
  
  # Port for the SOCKS5 proxy (default: 1080)
  socks5_port: 1080
  
  # Port for the HTTP proxy (default: 8080)
  http_port: 8080
  
  # Global limit on concurrent client connections (default: 1000)
  max_connections: 1000
  
  # Timeout in seconds for client connections (default: 300)
  connection_timeout: 300
  
  # Buffer size in bytes for read/write streams (default: 65536)
  buffer_size: 65536
```

---

### 2. Logging Settings (`logging`)

Controls log filtering and output targets (console, file, and systemd journald).

```yaml
logging:
  # Log filter level: trace, debug, info, warn, error (default: info)
  level: "info"
  
  # Enable logging stdout/stderr to console (default: true)
  console: true
  
  # Path to log file (optional; e.g. "/var/log/rust-socksd/rust-socksd.log")
  file: null
  
  # Enable native logging to systemd journald (default: false)
  journald: false
```

---

### 3. Security & Access Control (`security`)

Enforces source restrictions, destination filtering, and rate limiting.

```yaml
security:
  # Ingress source IPs/CIDR blocks allowed to connect (default: ["0.0.0.0/0"])
  # An empty list rejects all connections. Note: '0.0.0.0/0' is IPv4 only.
  allowed_networks:
    - "192.168.1.0/24"
    - "10.0.0.0/8"
    
  # Hostnames or subdomain suffixes to block (exact or suffix match)
  blocked_domains:
    - "malicious-site.com"
    - "blocked-domain.net"
    
  # Egress destination networks allowed (optional)
  # If specified, connections to targets outside these networks will be blocked.
  allowed_egress_networks:
    - "8.8.8.8"
    - "8.8.4.4"
    
  # Egress destination networks to block (optional)
  blocked_egress_networks:
    - "127.0.0.0/8"
    - "10.0.0.0/8"
    
  # Maximum allowed request size in bytes (default: 1048576 [1MB])
  max_request_size: 1048576
  
  # Rate limiting configurations (optional token-bucket scheme)
  rate_limit:
    requests_per_minute: 1000
    burst_size: 100
```

#### Enforcement Behavior:
- **`allowed_networks`**: Checked against client's IP upon handshake. An empty list rejects all. To allow all IP versions, include `0.0.0.0/0` and `::/0`.
- **`blocked_domains`**: Checks hostnames in proxy requests. If a requested domain matches or ends with an entry (e.g. `evil.com` will also block `sub.evil.com`), the proxy request is denied.
- **`rate_limit`**: Uses a per-source-IP token bucket refilled at the configured `requests_per_minute` rate with the defined `burst_size` capacity.
- **Egress Filtering**: Target IP resolution happens before connecting. Egress rules validate the resolved IP. If a destination violates the egress policies, the connection is blocked with a protocol-specific error.

---

### 4. Upstream Proxy Configuration (`upstream`)

Chain outgoing client traffic through an external proxy.

```yaml
upstream:
  # Enable proxy chaining (default: false)
  enabled: true
  
  # Upstream protocol: 'socks5' or 'http'
  protocol: socks5
  
  # Address of the upstream proxy
  address: "127.0.0.1"
  
  # Port of the upstream proxy
  port: 1080
  
  # Credentials for upstream authentication (optional basic auth)
  username: "proxy_user"
  password: "proxy_password"
  
  # Networks to exclude from upstream routing (bypass rules)
  exclude_networks:
    - "127.0.0.1/8"
    - "10.0.0.0/8"
    
  # Domains to exclude from upstream routing (bypass rules)
  exclude_domains:
    - "localhost"
    - "local.lan"
    
  # Precedence check: prioritize standard environment variables (default: true)
  prefer_env: true
```

#### Upstream Routing Logic:
- If `prefer_env` is `true`, standard proxy environment variables take precedence over the YAML configurations.
- Wildcards `*` in the exclusion networks/domains (or `NO_PROXY` environment variable) will bypass the upstream proxy for all requests.

---

## Environment Variable Overrides

Any command line execution of `rust-socksd` will check for specific environment variables. These take precedence over YAML configuration values, but are overridden by direct CLI options:

* **`RUST_SOCKSD_BIND_ADDRESS`**: Override the bind address (e.g., `0.0.0.0`)
* **`RUST_SOCKSD_SOCKS5_PORT`**: Override the SOCKS5 port (e.g., `1081`)
* **`RUST_SOCKSD_HTTP_PORT`**: Override the HTTP proxy port (e.g., `8081`)
* **`RUST_SOCKSD_LOG_LEVEL`**: Override the log level (`trace`, `debug`, `info`, `warn`, `error`)
* **`RUST_SOCKSD_ADMIN_PORT`**: Override the Admin API port (e.g., `8082`)
* **`RUST_SOCKSD_ADMIN_ENABLED`**: Override whether the Admin API is enabled (`true`/`false`)
