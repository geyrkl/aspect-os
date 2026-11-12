#!/bin/bash
# ASPECT OS - Rust-Toolchain installieren (falls fehlt)
set -e
if [ ! -x /root/.cargo/bin/rustc ]; then
  export DEBIAN_FRONTEND=noninteractive
  command -v curl >/dev/null || apt-get install -y -qq curl >/dev/null
  curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal --default-toolchain stable
fi
/root/.cargo/bin/rustup target add x86_64-unknown-uefi x86_64-unknown-none
/root/.cargo/bin/rustc --version
/root/.cargo/bin/rustup target list --installed
echo "RUST_OK"
