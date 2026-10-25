# Installation

## Requirements

| Requirement | Minimum version |
|-------------|----------------|
| Rust toolchain | 1.80 |
| C compiler (`gcc` / `clang`) | any recent |
| OpenSSL development headers | 1.1 / 3.x |
| `pkg-config` | any |

## From source (without Nix)

```bash
git clone https://github.com/affolter-engineering/mqattack.git
cd mqattack
cargo build --release
```

The compiled binary is placed at `target/release/mqattack`.

Copy it to a location on your `$PATH`:

```bash
install -m 755 target/release/mqattack ~/.local/bin/
```

## From source (with Nix)

A `shell.nix` is provided that pins all build dependencies:

```bash
nix-shell
cargo build --release
```

## Nixpkgs

For Nix and NixOS users, mqattack is packaged in nixpkgs. The latest releases
may only be present in the `unstable` channel.

```bash
nix-env -iA nixos.mqattack
```

Or declaratively in `configuration.nix`:

```nix
environment.systemPackages = [ pkgs.mqattack ];
```

## Verify the installation

```bash
mqattack --version
mqattack --help
```
