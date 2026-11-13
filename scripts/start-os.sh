#!/bin/bash
# ASPECT OS - start live (window via WSLg)
# NOTE: under WSLg the gtk backend cannot grab the pointer, so the guest
# never sees relative mouse motion. sdl handles this, vnc is the fallback.
CODE=/usr/share/OVMF/OVMF_CODE_4M.fd
VARS=/tmp/aspect-os-vars.fd
cp /usr/share/OVMF/OVMF_VARS_4M.fd "$VARS" 2>/dev/null || cp "$VARS" "$VARS"
DISP="${ASPECT_DISP:-sdl}"
exec qemu-system-x86_64 \
  -enable-kvm -m 2048 -cpu host -smp 2 \
  -drive if=pflash,format=raw,readonly=on,file="$CODE" \
  -drive if=pflash,format=raw,file="$VARS" \
  -drive file="/mnt/c/Klemens/Coding/Aspect OS/build/aspect-os.iso",media=cdrom,readonly=on \
  -device ich9-ahci,id=ahci0 \
  -drive file="/mnt/c/Klemens/Coding/Aspect OS/build/aspect-data.img",format=raw,if=none,id=aspfs \
  -device ide-hd,drive=aspfs,bus=ahci0.0 \
  -nic user,model=e1000 \
  -display "$DISP"
