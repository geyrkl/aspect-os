#!/bin/bash
# ASPECT OS - live starten (Fenster via WSLg)
CODE=/usr/share/OVMF/OVMF_CODE_4M.fd
VARS=/tmp/aspect-os-vars.fd
cp /usr/share/OVMF/OVMF_VARS_4M.fd "$VARS" 2>/dev/null || cp "$VARS" "$VARS"
exec qemu-system-x86_64 \
  -enable-kvm -m 2048 -cpu host -smp 2 \
  -drive if=pflash,format=raw,readonly=on,file="$CODE" \
  -drive if=pflash,format=raw,file="$VARS" \
  -drive file="/mnt/c/Klemens/Coding/Aspect OS/build/aspect-os.iso",media=cdrom,readonly=on \
  -display gtk
