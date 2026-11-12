#!/bin/bash
# ASPECT OS - WSL-Build-Einstiegspunkt (Rust-Toolchain + make)
set -e
if [ ! -x /root/.cargo/bin/rustc ]; then
  export DEBIAN_FRONTEND=noninteractive
  command -v curl >/dev/null || apt-get install -y -qq curl >/dev/null
  curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal --default-toolchain stable
fi
command -v x86_64-w64-mingw32-gcc >/dev/null || {
  apt-get update -qq
  apt-get install -y -qq build-essential gcc nasm xorriso mtools \
    qemu-system-x86 ovmf imagemagick gcc-mingw-w64-x86-64 \
    binutils-x86-64-linux-gnu >/dev/null
}
/root/.cargo/bin/rustup target add x86_64-unknown-uefi x86_64-unknown-none >/dev/null
cd "/mnt/c/Klemens/Coding/Aspect OS"
make "$@"
