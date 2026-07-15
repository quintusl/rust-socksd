# Admin API Reference Guide

The administrative server allows monitoring the proxy server status, validating and reloading configurations at runtime, and inspecting execution metrics.

## Configuration

To enable the admin server, append an `admin` section to your main `config.yml` configuration file:

```yaml
admin:
  # Enable/disable the admin API listener (default: false)
  enabled: true
  
  # Address to bind the listener to (default: 127.0.0.1)
  bind_address: "127.0.0.1"
  
  # Port to listen on (default: 8081)
  port: 8081
  
  # Static pre-shared token (Optional)
  # If configured, clients can bypass login and authenticate directly
  token: "my-secure-static-bypass-token"
  
  # List of usernames permitted to log in and receive transient tokens
  admin_users:
    - "admin"
    
  # Lifespan of transient dynamic tokens in seconds (default: 3600)
  token_ttl: 3600
```

---

## Authentication Schemes

The Admin API supports two primary authentication modes for all administrative endpoints (except `/health`):

### 1. Static Token Authentication
Use the token configured via `admin.token` as a direct Bearer header:
```bash
Authorization: Bearer my-secure-static-bypass-token
```

### 2. Dynamic Token Authentication
If a static token is not preferred, exchange credentials for a temporary session token using Basic Authentication:
1. Call `/login` using the credentials of a user defined in your active authenticator backend (e.g. `users.yml`, LDAP, Database) who is listed in the config's `admin.admin_users`.
2. Extract the returned `token` from the response.
3. Attach it to administrative calls:
   ```bash
   Authorization: Bearer <returned-token>
   ```

---

## Endpoints

### 1. Liveness & Health Check
Checks if the admin server is up. This endpoint is **public** and does not require authentication headers.

* **Path:** `GET /health`
* **Response Status:** `200 OK`
* **Response Body:**
  ```json
  {
    "status": "ok"
  }
  ```
* **Example Request:**
  ```bash
  curl http://127.0.0.1:8081/health
  ```

---

### 2. User Authentication / Login
Exchanges user credentials for a dynamic bearer token.

* **Path:** `POST /login`
* **Authentication:** HTTP Basic Auth (username/password)
* **Response Status:** `200 OK` on success, `401 Unauthorized` on bad credentials, `403 Forbidden` if the user is authenticated but not configured under `admin_users`.
* **Response Body:**
  ```json
  {
    "token": "QWVyV1p2T2x3NmRl...",
    "expires_in": 3600
  }
  ```
* **Example Request:**
  ```bash
  curl -X POST -u "admin:secretpassword" http://127.0.0.1:8081/login
  ```

---

### 3. Metrics Export
Provides execution counters and gauges in Prometheus-compatible exposition format.

* **Path:** `GET /metrics`
* **Authentication:** Bearer token
* **Response Status:** `200 OK`
* **Response Body:**
  ```prometheus
  # HELP rust_socksd_active_connections Number of active connections
  # TYPE rust_socksd_active_connections gauge
  rust_socksd_active_connections 2
  # HELP rust_socksd_total_connections Total connections accepted
  # TYPE rust_socksd_total_connections counter
  rust_socksd_total_connections 14
  # HELP rust_socksd_bytes_tx Total bytes transmitted (client to target)
  # TYPE rust_socksd_bytes_tx counter
  rust_socksd_bytes_tx 819240
  # HELP rust_socksd_bytes_rx Total bytes received (target to client)
  # TYPE rust_socksd_bytes_rx counter
  rust_socksd_bytes_rx 1902488
  # HELP rust_socksd_auth_failures Total authentication failures
  # TYPE rust_socksd_auth_failures counter
  rust_socksd_auth_failures 1
  ```
* **Example Request:**
  ```bash
  curl -H "Authorization: Bearer QWVyV1p2T2x3NmRl..." http://127.0.0.1:8081/metrics
  ```

---

### 4. Configuration Inspect
Returns the active configuration schema. Sensitive values (like passwords, keys, and URL credentials) are automatically masked to `******`.

* **Path:** `GET /config`
* **Authentication:** Bearer token
* **Response Status:** `200 OK`
* **Response Body:**
  ```json
  {
    "server": {
      "bind_address": "127.0.0.1",
      "socks5_port": 1080,
      "http_port": 8080,
      "max_connections": 1000,
      "connection_timeout": 300,
      "buffer_size": 65536
    },
    "auth": {
      "enabled": true,
      "backend": {
        "type": "simple",
        "user_config_file": "config/users.yml"
      }
    },
    ...
  }
  ```
* **Example Request:**
  ```bash
  curl -H "Authorization: Bearer QWVyV1p2T2x3NmRl..." http://127.0.0.1:8081/config
  ```

---

### 5. Configuration Validation
Validates a potential configuration without applying it.

* **Path:** `POST /config/validate`
* **Authentication:** Bearer token
* **Request Body:** Configuration YAML string
* **Response Status:** `200 OK` (if valid), `400 Bad Request` (if validation rules or parsing failed)
* **Response Body (Valid):**
  ```json
  {
    "valid": true
  }
  ```
* **Response Body (Invalid):**
  ```json
  {
    "valid": false,
    "error": "SOCKS5 and HTTP ports cannot be the same"
  }
  ```
* **Example Request:**
  ```bash
  curl -H "Authorization: Bearer QWVyV1p2T2x3NmRl..." \
       -H "Content-Type: application/yaml" \
       --data-binary @config.yml \
       http://127.0.0.1:8081/config/validate
  ```

---

### 6. Live Configuration Reload
Triggers the server to re-read its main configuration file from disk. Swaps out the routing, rules, and authenticators seamlessly.

* **Path:** `POST /config/reload`
* **Authentication:** Bearer token
* **Response Status:** `200 OK` on success, `400 Bad Request` or `500 Internal Error` on failure (e.g. invalid syntax, file missing, or unsupported port updates).
* **Response Body (Success):**
  ```json
  {
    "status": "reloaded"
  }
  ```
* **Example Request:**
  ```bash
  curl -X POST -H "Authorization: Bearer QWVyV1p2T2x3NmRl..." http://127.0.0.1:8081/config/reload
  ```

> [!IMPORTANT]
> To ensure continuous operation, you **cannot** update bind addresses or ports (for SOCKS5, HTTP, or Admin listeners) at runtime via a reload command. Attempting to change bind targets will abort the reload operation with a validation error, requesting a service restart instead.
