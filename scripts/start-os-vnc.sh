#!/bin/bash
# ASPECT OS - start headless + VNC (fallback when sdl/grab does not work
# under WSLg). Connect from Windows with any VNC viewer to 127.0.0.1:5901.
CODE=/usr/share/OVMF/OVMF_CODE_4M.fd
VARS=/tmp/aspect-os-vars.fd
cp /usr/share/OVMF/OVMF_VARS_4M.fd "$VARS" 2>/dev/null || cp "$VARS" "$VARS"

# stop a previous instance
if [ -f /tmp/aspect-os.pid ]; then
  kill "$(cat /tmp/aspect-os.pid)" 2>/dev/null || true
  sleep 1
fi

qemu-system-x86_64 \
  -enable-kvm -m 2048 -cpu host -smp 2 \
  -drive if=pflash,format=raw,readonly=on,file="$CODE" \
  -drive if=pflash,format=raw,file="$VARS" \
  -drive file="/mnt/c/Klemens/Coding/Aspect OS/build/aspect-os.iso",media=cdrom,readonly=on \
  -device ich9-ahci,id=ahci0 \
  -drive file="/mnt/c/Klemens/Coding/Aspect OS/build/aspect-data.img",format=raw,if=none,id=aspfs \
  -device ide-hd,drive=aspfs,bus=ahci0.0 \
  -display none \
  -vnc 127.0.0.1:1 \
  -pidfile /tmp/aspect-os.pid \
  -daemonize
echo "ASPECT OS running - connect a VNC viewer to 127.0.0.1:5901"
echo "stop with: kill \$(cat /tmp/aspect-os.pid)"
