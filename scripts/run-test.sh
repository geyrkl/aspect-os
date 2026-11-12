#!/bin/bash
# ASPECT OS - QEMU-Test im WSL aufrufen (Screendump landet in docs/screenshots/)
cd "/mnt/c/Klemens/Coding/Aspect OS" || exit 1
bash scripts/run-qemu.sh 2>&1 | tail -30
