{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  name = "mqattack-dev";

  buildInputs = with pkgs; [
    # Rust toolchain
    rustc
    cargo
    clippy
    rustfmt

    # C compiler and linker required by Cargo build scripts and native crates
    gcc
    pkg-config

    # OpenSSL (used by rumqttc's TLS support)
    openssl.dev
    openssl
  ];

  shellHook = ''
    export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig:$PKG_CONFIG_PATH"
    export OPENSSL_DIR="${pkgs.openssl.dev}"
    export OPENSSL_LIB_DIR="${pkgs.openssl.out}/lib"
    export OPENSSL_INCLUDE_DIR="${pkgs.openssl.dev}/include"

    echo "mqattack dev shell ready - $(rustc --version)"
  '';
}
