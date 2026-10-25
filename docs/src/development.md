# Development

## Repository layout

```text
mqattack/
├── Cargo.toml          # Package metadata and dependencies
├── Cargo.lock          # Locked dependency versions
├── shell.nix           # Nix development environment
├── book.toml           # mdBook configuration
├── src/
│   ├── main.rs         # CLI entry point and subcommand router
│   ├── mqtt/
│   │   ├── mod.rs
│   │   └── client.rs   # Connection options, TLS setup, event loop
│   └── commands/
│       ├── mod.rs
│       ├── subscribe.rs
│       ├── publish.rs
│       ├── fingerprint.rs
│       ├── sys_info.rs
│       ├── check_acl.rs
│       ├── brute_auth.rs
│       ├── enum_topics.rs
│       ├── enum_perms.rs
│       ├── will_inject.rs
│       └── payload_inject.rs
└── docs/
    └── src/            # mdBook source (this documentation)
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `rumqttc` | Async MQTT 3.1.1 client |
| `clap` | CLI argument parsing (derive macros) |
| `tokio` | Async runtime |
| `anyhow` | Error handling with context |
| `chrono` | Timestamp formatting |
| `hex` | Hexadecimal encoding for binary payloads |
| `rustls` | TLS implementation |
| `rustls-pemfile` | PEM certificate and key parsing |
| `rustls-native-certs` | System root CA loading |

## Setting up a development environment

### With Nix

```bash
nix-shell        # activates rustc, cargo, clippy, rustfmt, gcc, pkg-config, openssl
cargo build
```

### Without Nix

Install the Rust toolchain (1.80+), a C compiler, and OpenSSL development
headers, then:

```bash
cargo build
```

## Building

```bash
# Debug build (faster compile, slower binary)
cargo build

# Release build
cargo build --release
```

## Running tests

```bash
cargo test
```

All tests are unit tests embedded in their respective modules with
`#[cfg(test)]` blocks.  There are no external dependencies or network calls
in the test suite.

## Linting and formatting

```bash
cargo clippy -- -D warnings
cargo fmt --check
```

## Code style

The project follows standard Rust idioms:

- Async/await with `tokio` throughout
- `anyhow::Result` for error propagation
- `clap` derive macros for all CLI structs
- Each command lives in its own module under `src/commands/`
- Shared connection logic lives in `src/mqtt/client.rs`

## Adding a new command

1. Create `src/commands/my_command.rs` with a `pub async fn run(...)` entry
   point that accepts the shared `ConnectionArgs` and any command-specific args.
2. Add the module to `src/commands/mod.rs`.
3. Define a clap `Args` struct in your new module.
4. Add the subcommand variant to the `Commands` enum in `src/main.rs`.
5. Route it in the `match` block in `main`.

## Local broker for testing

The quickest way to spin up a test broker is with Docker:

```bash
docker run -it --rm -p 1883:1883 eclipse-mosquitto
```

For a broker with authentication:

```bash
# Create a password file
docker run --rm eclipse-mosquitto \
  mosquitto_passwd -c -b /tmp/passwd testuser testpass

docker run -it --rm -p 1883:1883 \
  -v /tmp/passwd:/mosquitto/config/passwd \
  eclipse-mosquitto mosquitto -c /mosquitto/config/mosquitto.conf
```

## Building the documentation

This documentation is built with [mdBook](https://rust-lang.github.io/mdBook/).

```bash
# Install mdBook
cargo install mdbook

# Serve with live reload
mdbook serve docs/

# Build static HTML
mdbook build docs/
```

The generated HTML is placed in `docs/book/`.

## Security disclosures

Report vulnerabilities to **mail@fabian-affolter.ch**.

GPG key fingerprint: `2F6C 930F D3C4 7E38 6AFA 4EB4 E23C D2DD 36A4 397F`

See [SECURITY.md](https://github.com/affolter-engineering/mqattack/blob/main/SECURITY.md)
for the full policy.
