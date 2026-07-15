# Troubleshooting Guide

This guide helps resolve common issues encountered while deploying or running `rust-socksd`.

---

## Common Issues

### 1. Permission Denied

* **Symptoms**: The server crashes on startup with a filesystem read or write error, or fails to load `users.yml` or `config.yml`.
* **Causes**: The dedicated service user (e.g., `rust-socksd` or Docker's `appuser`) does not have read permissions for the configuration files or write permissions for the log directories.
* **Solutions**:
  - Grant read permissions on configuration files to the running user:
    ```bash
    sudo chown -R rust-socksd:rust-socksd /etc/rust-socksd
    sudo chmod 640 /etc/rust-socksd/config.yml
    ```
  - For Docker, ensure the mounted directory has the correct user ownership matching UID/GID `1001`.

### 2. Port Already in Use

* **Symptoms**: Startup failure with: `Address already in use` (OS Error 98 or 48).
* **Causes**: Another service (or an existing instance of `rust-socksd`) is already bound to the requested port (SOCKS5 `1080`, HTTP `8080`, or Admin `8081`).
* **Solutions**:
  - Identify the process utilizing the port:
    ```bash
    # Linux
    sudo ss -lntp | grep -E '1080|8080|8081'
    # macOS
    sudo lsof -i -P -n | grep -E '1080|8080|8081'
    ```
  - Change the ports in `config.yml` or supply port override flags (e.g. `--socks5-port 1082`).

### 3. Connection Refused

* **Symptoms**: Clients cannot connect to the proxy and report "Connection Refused".
* **Causes**:
  - The proxy is configured to bind to `127.0.0.1` and the client is attempting to connect from a different network interface.
  - A local firewall (like `ufw`, `iptables`, or macOS Application Firewall) is blocking the ports.
* **Solutions**:
  - Set `server.bind_address` to `0.0.0.0` in `config.yml` to listen on all interfaces.
  - Allow proxy ports through your firewall:
    ```bash
    # UFW (Ubuntu/Debian)
    sudo ufw allow 1080/tcp
    sudo ufw allow 8080/tcp
    ```

### 4. Authentication Failures

* **Symptoms**: Client requests fail with `407 Proxy Authentication Required` (HTTP) or connection close (SOCKS5).
* **Causes**:
  - Password hashes mismatch.
  - Configuration file path for `user_config_file` is incorrect or unreadable.
  - External auth backends (LDAP, Database) are unreachable or search queries are invalid.
* **Solutions**:
  - For the `simple` backend, check user lists:
    ```bash
    rust-socksd user --user-config users.yml list
    ```
  - For LDAP or DB backends, inspect the database logs or verify query logic against the schema (e.g., column names and parameter placeholders `?` vs `$1` depending on db type).

---

## Debugging

Enable verbose debug or trace logging to see detailed logs of connection processing:

### Via Command Line

Pass the `-v` (debug) or `-vv` (trace) verbose flags:

```bash
rust-socksd --config config.yml -v
```

### Via Configuration

Update the `logging` block in `config.yml`:

```yaml
logging:
  level: "debug" # or "trace"
  console: true
```

---

## Log Inspection

### Systemd Service

For systemd daemon deployments, stream live service logs:

```bash
sudo journalctl -u rust-socksd -f
```

### Log File

If file logging is configured, tail the log file:

```bash
tail -f /var/log/rust-socksd/rust-socksd.log
```
