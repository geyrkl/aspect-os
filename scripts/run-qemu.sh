#!/bin/bash
# ASPECT OS - QEMU test with OVMF (UEFI) + QMP screendump
# runtime artifacts (socket/pid/screens) live in the Linux FS, results go to C:
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
  -device ich9-ahci,id=ahci0 \
  -drive file=build/aspect-data.img,format=raw,if=none,id=aspfs \
  -device ide-hd,drive=aspfs,bus=ahci0.0 \
  -serial file:"$OUT/serial.log" \
  -display none \
  -qmp unix:"$OUT/qmp.sock",server,nowait \
  -pidfile "$OUT/qemu.pid" \
  -daemonize
sleep 2
echo "[QEMU] PID: $(cat "$OUT/qemu.pid")"
echo "[QEMU] waiting 15s for desktop ..."
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
time.sleep(3.2)
s.sendall(b'{"execute":"screendump","arguments":{"filename":"' + out.encode() + b'/02-clock.ppm"}}\n')
rd()

# --- M3 test: move mouse + click, type keys ---
def inp(events):
    s.sendall(b'{"execute":"input-send-event","arguments":{"events":' + events + b'}}\n')
    rd()

# move the cursor from center to bottom right (pauses keep the ps2 queue in sync)
inp(b'[{"type":"rel","data":{"axis":"x","value":150}}]')
time.sleep(0.2)
inp(b'[{"type":"rel","data":{"axis":"y","value":100}}]')
time.sleep(0.2)
inp(b'[{"type":"rel","data":{"axis":"x","value":60}}]')
time.sleep(0.2)
inp(b'[{"type":"rel","data":{"axis":"y","value":40}}]')
time.sleep(0.6)
# left click
inp(b'[{"type":"btn","data":{"down":true,"button":"left"}}]')
inp(b'[{"type":"btn","data":{"down":false,"button":"left"}}]')
time.sleep(0.6)
# keyboard: q, e, v (send-key -> i8042 -> IRQ1)
for k in (b'q', b'e', b'v'):
    s.sendall(b'{"execute":"send-key","arguments":{"keys":[{"type":"qcode","data":"' + k + b'"}]}}\n')
    rd()
    time.sleep(0.3)
time.sleep(1.0)
s.sendall(b'{"execute":"screendump","arguments":{"filename":"' + out.encode() + b'/03-input.ppm"}}\n')
rd()
time.sleep(1.5)
PYEOF

if [ -f "$OUT/01-desktop.ppm" ]; then
  convert "$OUT/01-desktop.ppm" "$OUT/01-desktop.png"
  rm -f "$OUT/01-desktop.ppm"
fi
if [ -f "$OUT/02-clock.ppm" ]; then
  convert "$OUT/02-clock.ppm" "$OUT/02-clock.png"
  rm -f "$OUT/02-clock.ppm"
fi
if [ -f "$OUT/03-input.ppm" ]; then
  convert "$OUT/03-input.ppm" "$OUT/03-input.png"
  rm -f "$OUT/03-input.ppm"
fi
mkdir -p /mnt/c/Klemens/Coding/Aspect\ OS/docs/screenshots
cp -f "$OUT/01-desktop.png" /mnt/c/Klemens/Coding/Aspect\ OS/docs/screenshots/ 2>/dev/null || true
cp -f "$OUT/02-clock.png" /mnt/c/Klemens/Coding/Aspect\ OS/docs/screenshots/ 2>/dev/null || true
cp -f "$OUT/03-input.png" /mnt/c/Klemens/Coding/Aspect\ OS/docs/screenshots/ 2>/dev/null || true
echo "--- serial.log ---"
cat "$OUT/serial.log" 2>/dev/null || echo "(no serial.log)"
ls -la "$OUT"
