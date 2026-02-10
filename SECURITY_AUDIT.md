# Security Audit Report for rust-socksd

**Date**: 2026-02-10
**Target**: rust-socksd
**Auditor**: Antigravity

## Executive Summary

A security audit was performed on the `rust-socksd` codebase. The audit identified **1 Critical**, **1 High**, and **1 Medium** severity vulnerabilities.

The most critical issue is that the HTTP proxy component is completely unauthenticated, effectively acting as an open proxy regardless of the configuration settings. Additionally, a significant Denial of Service (DoS) vector exists in the SOCKS5 authentication mechanism, which triggers file I/O operations for every authentication attempt.

## Findings

### 1. Critical: HTTP Proxy Authentication Bypass (Open Proxy)

**Severity**: Critical
**Location**: `src/server.rs`, `src/http_proxy.rs`

**Description**:
The HTTP proxy implementation completely ignores the global authentication configuration. In `src/server.rs`, the `handle_http_connection` function ignores the passed `Config` object. Furthermore, `src/http_proxy.rs` contains no logic to check for `Proxy-Authorization` headers or validate credentials.

This means that even if the details in `config.yml` enable authentication, the HTTP proxy (port 8080 by default) remains open to the public. Anyone can use this proxy to relay traffic, potentially masking malicious activities or bypassing IP-based restrictions.

**Code Reference**:
```rust
// src/server.rs:214
async fn handle_http_connection(mut stream: TcpStream, _config: Arc<Config>) -> Result<()> {
    // _config is ignored, auth settings are never checked
    let handler = HttpProxyHandler;
    // ...
}
```

**Recommendation**:
- Pass the `Config` object to `HttpProxyHandler`.
- Implement Basic Authentication parsing in `http_proxy.rs`.
- Enforce authentication logic similar to the SOCKS5 handler.

### 2. High: Denial of Service via Authentication I/O Exhaustion

**Severity**: High
**Location**: `src/config.rs`, `src/socks5.rs`

**Description**:
The application reloads the user configuration file from disk **on every single authentication attempt**.
The `validate_user` function in `Config` calls `UserConfig::load_from_file`.

```rust
// src/config.rs:171
let user_config = UserConfig::load_from_file(user_config_path)?;
```

An attacker can flood the server with authentication requests (e.g., using random credentials). This will force the server to perform blocking file I/O operations for every request, rapidly exhausting system I/O resources and CPU, leading to a Denial of Service.

**Recommendation**:
- Load `UserConfig` once during application startup.
- Store the `UserConfig` in memory (e.g., inside the `Config` struct).
- Remove the file load operation from the hot path of authentication verification.

### 3. Medium: Unbounded Memory Allocation in HTTP Parsing

**Severity**: Medium
**Location**: `src/http_proxy.rs`

**Description**:
The `handle_request` function in `src/http_proxy.rs` uses `read_line` to read HTTP headers without enforcing any size limits.

```rust
// src/http_proxy.rs:80
buf_reader.read_line(&mut line).await?;
```

By sending an HTTP request with an extremely long line or an infinite number of headers, an attacker can cause the server to allocate excessive memory, leading to an Out-Of-Memory (OOM) crash.
The `max_request_size` setting from `Config` is not utilized in the HTTP proxy handler.

**Recommendation**:
- Use `take()` to limit the number of bytes read.
- Enforce `max_request_size` during header parsing.
- Limit the maximum number of headers allowed.

### 4. Medium: Unbounded Connection Acceptance

**Severity**: Medium
**Location**: `src/server.rs`

**Description**:
The server accepts TCP connections immediately and spawns a tokio task before acquiring a semaphore permit. 

```rust
// src/server.rs:69-77
match listener.accept().await {
    Ok((stream, addr)) => {
        tokio::spawn(async move {
            let _permit = match semaphore.acquire().await {
```

If the semaphore is full (max connections reached), new connections are still accepted and tasks are spawned, which then wait on the semaphore. An attacker can open thousands of connections, exhausting file descriptors and memory for the waiting tasks, even if the "active" connection limit is respected.

**Recommendation**:
- Implement a mechanism to stop accepting new connections when the semaphore is full, or impose a hard limit on the total number of open file descriptors/tasks.

## Conclusion

The application requires immediate remediation, particularly for the HTTP Proxy authentication bypass and the authentication performance issue.
