#!/bin/bash
# ASPECT OS - run the QEMU test from WSL (screendump lands in docs/screenshots/)
cd "/mnt/c/Klemens/Coding/Aspect OS" || exit 1
bash scripts/run-qemu.sh 2>&1 | tail -30
