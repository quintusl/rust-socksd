# Deployment & Packaging Guide

This guide describes how to deploy `rust-socksd` in production environments, run it as a systemd service, and package it for Debian and Arch Linux systems.

---

## Systemd Service

When installed via package managers, a systemd service file is registered. It provides daemon management, automatic crash restarts, journald log routing, and security sandbox features.

### Security Hardening in Systemd

The service file includes:
* **User Isolation**: Runs under a dedicated unprivileged user account `rust-socksd`.
* **Privilege Restrictions**: Disallows new privilege flags (`NoNewPrivileges=yes`).
* **Sandbox Protections**: Read-only access to most of the filesystem, restricted `/tmp` (`PrivateTmp=yes`), and limited device access.

### Service Commands

Use standard `systemctl` commands to manage the daemon:

```bash
# Enable the service to launch on boot
sudo systemctl enable rust-socksd

# Start the proxy server immediately
sudo systemctl start rust-socksd

# Restart the service (required after changing bind ports)
sudo systemctl restart rust-socksd

# Check runtime status
sudo systemctl status rust-socksd
```

### Accessing Service Logs

Logs are routed directly to systemd's journal. View them with `journalctl`:

```bash
# Stream live logs
sudo journalctl -u rust-socksd -f

# Filter logs by priority (e.g. warnings and errors)
sudo journalctl -u rust-socksd -p err..warn
```

---

## Building Debian Packages

You can build a native Debian `.deb` package to deploy onto Debian/Ubuntu machines.

### Build Dependencies

Install required build utilities and libraries:

```bash
sudo apt-get update
sudo apt-get install debhelper-compat cargo rustc libpam0g-dev
```

### Packaging Command

Run the package build tools from the project root directory:

```bash
# Build the binary and bundle it into a deb package
dpkg-buildpackage -b -uc -us
```

This generates a `.deb` package in the parent directory, which you can install via `dpkg -i`.

---

## Arch Linux Package (AUR)

For Arch Linux, `rust-socksd` is available in the Arch User Repository (AUR).

### Manual Build

To compile and package manually using `PKGBUILD`:

```bash
# Clone the AUR repository
git clone https://aur.archlinux.org/rust-socksd.git
cd rust-socksd

# Build and install the package with dependencies
makepkg -si
```
