#!/bin/bash
# ASPECT OS - QEMU-Test mit OVMF (UEFI) + QMP-Screendump
# Runtime-Artefakte (Socket/PID/Screens) liegen im Linux-FS, Ergebnisse nach C:
set -e
cd "$(dirname "$0")/.."
OUT=/root/aspect-qemu
mkdir -p "$OUT"

if [ -f "$OUT/qemu.pid" ]; then
  kill "$(cat "$OUT/qemu.pid")" 2>/dev/null || true
  sleep 1
fi
rm -f "$OUT/qmp.sock" "$OUT"/*.ppm "$OUT"/*.png "$OUT/serial.log"

CODE=/usr/share/OVMF/OVMF_CODE_4M.fd
cp /usr/share/OVMF/OVMF_VARS_4M.fd "$OUT/VARS.fd"

qemu-system-x86_64 \
  -enable-kvm -m 2048 -cpu host -smp 2 \
  -drive if=pflash,format=raw,readonly=on,file="$CODE" \
  -drive if=pflash,format=raw,file="$OUT/VARS.fd" \
  -drive file=build/aspect-os.iso,media=cdrom,readonly=on \
  -serial file:"$OUT/serial.log" \
  -display none \
  -qmp unix:"$OUT/qmp.sock",server,nowait \
  -pidfile "$OUT/qemu.pid" \
  -daemonize
sleep 2
echo "[QEMU] PID: $(cat "$OUT/qemu.pid")"
echo "[QEMU] warte 15s auf Desktop ..."
sleep 15

python3 - "$OUT" <<'PYEOF'
import socket, sys, time, json
out = sys.argv[1]
s = socket.socket(socket.AF_UNIX); s.connect(out + "/qmp.sock")
s.settimeout(3)
def rd():
    buf = b''
    while True:
        try: buf += s.recv(65536)
        except socket.timeout: break
        try:
            msg = json.loads(buf.decode())
            if 'return' in msg or 'error' in msg: return msg
        except Exception: pass
rd()
s.sendall(b'{"execute":"qmp_capabilities"}\n'); rd()
s.sendall(b'{"execute":"screendump","arguments":{"filename":"' + out.encode() + b'/01-desktop.ppm"}}\n')
rd()
time.sleep(1.5)
PYEOF

if [ -f "$OUT/01-desktop.ppm" ]; then
  convert "$OUT/01-desktop.ppm" "$OUT/01-desktop.png"
  rm -f "$OUT/01-desktop.ppm"
fi
mkdir -p /mnt/c/Klemens/Coding/Aspect\ OS/docs/screenshots
cp -f "$OUT/01-desktop.png" /mnt/c/Klemens/Coding/Aspect\ OS/docs/screenshots/ 2>/dev/null || true
echo "--- serial.log ---"
cat "$OUT/serial.log" 2>/dev/null || echo "(kein serial.log)"
ls -la "$OUT"
