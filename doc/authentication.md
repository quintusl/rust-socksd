# Authentication Backends

`rust-socksd` supports multiple authentication backends to secure your SOCKS5 and HTTP proxy endpoints. When authentication is enabled globally (`auth.enabled: true`), SOCKS5 requests require username/password authentication, and HTTP requests require HTTP Basic Authentication (via the `Proxy-Authorization` header).

---

## 1. Simple (File-based) Backend

This backend stores credentials in a local YAML configuration file. Passwords are securely hashed using Argon2, Bcrypt, or Scrypt.

### Configuration

```yaml
auth:
  enabled: true
  type: simple
  user_config_file: "config/users.yml"
```

### CLI User Management

You can manage this file dynamically using the `user` subcommand.

#### Initialize User Configuration

Create a new, empty user database file with a default hashing algorithm:

```bash
rust-socksd user --user-config config/users.yml init --hash-type argon2
```

*Options for `--hash-type`: `argon2` (default), `bcrypt`, `scrypt`*

#### Add a New User

Add a user. You will be prompted for a password if you do not specify it on the command line:

```bash
rust-socksd user --user-config config/users.yml add myuser
```

*Specify custom hash type per user:*
```bash
rust-socksd user --user-config config/users.yml add myuser --hash-type bcrypt
```

#### List Configured Users

List all users, their status (enabled/disabled), creation time, and last modified time:

```bash
rust-socksd user --user-config config/users.yml list
```

#### Update Password

```bash
rust-socksd user --user-config config/users.yml update myuser
```

#### Enable / Disable a User

```bash
# Disable a user
rust-socksd user --user-config config/users.yml enable myuser false

# Enable a user
rust-socksd user --user-config config/users.yml enable myuser true
```

#### Remove a User

```bash
rust-socksd user --user-config config/users.yml remove myuser
```

---

## 2. PAM Backend

Integrates authentication with the Linux Pluggable Authentication Modules (PAM) subsystem. This allows proxying to authenticate using local system user accounts.

> [!NOTE]
> Requires building `rust-socksd` with the `pam-auth` feature (enabled by default on Linux). The server process must also run with sufficient privileges (e.g., as `root`) or belong to the `shadow` group to read system user credentials.

### Configuration

```yaml
auth:
  enabled: true
  type: pam
  service: "socksd" # Refers to config file in /etc/pam.d/socksd
```

---

## 3. LDAP Backend

Authenticates users against an LDAP directory server (such as Active Directory or OpenLDAP).

### Configuration

```yaml
auth:
  enabled: true
  type: ldap
  url: "ldap://ldap.example.com:389"
  base_dn: "ou=users,dc=example,dc=com"
  # Optional: DN and password used to search for user entries (if anonymous binds are disabled)
  bind_dn: "cn=admin,dc=example,dc=com"
  bind_password: "admin_password"
  # User search query filter. '{}' is replaced by the logging-in username.
  user_filter: "(uid={})"
```

---

## 4. Database (SQL) Backend

Authenticates users by querying a database and verifying the password hash against secure algorithms. Supports MySQL/MariaDB and PostgreSQL.

### Configuration

```yaml
auth:
  enabled: true
  type: database
  db_type: "mysql" # or "postgres"
  url: "mysql://socksd_user:secret@localhost/socksd_db"
  # Query must return a single column containing the password hash
  query: "SELECT password_hash FROM users WHERE username = ?"
  # Hashing algorithm used in the database: argon2, bcrypt, or scrypt
  hash_type: "argon2"
```
