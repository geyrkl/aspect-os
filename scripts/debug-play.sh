#!/bin/bash
# ASPECT OS - repro: music + play click, frame-by-frame WINS trace
set -e
cd "/mnt/c/Klemens/Coding/Aspect OS"
OUT=/root/aspect-qemu
if [ -f "$OUT/qemu.pid" ]; then
  kill "$(cat "$OUT/qemu.pid")" 2>/dev/null || true
  sleep 1
fi
rm -f "$OUT/qmp.sock" "$OUT/serial.log"

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
  -nic user,model=e1000 \
  -serial file:"$OUT/serial.log" \
  -display none \
  -qmp unix:"$OUT/qmp.sock",server,nowait \
  -pidfile "$OUT/qemu.pid" \
  -daemonize
sleep 2
sleep 15

python3 - "$OUT" <<'PYEOF'
import socket, sys, time, json
out = sys.argv[1]
s = socket.socket(socket.AF_UNIX); s.connect(out + "/qmp.sock")
s.settimeout(5)

def rd():
    buf = b''
    while True:
        try: buf += s.recv(65536)
        except socket.timeout: break
        try:
            m = json.loads(buf.decode())
            if 'return' in m or 'error' in m: return m
        except Exception: pass

def cmd(o):
    s.sendall(json.dumps(o).encode() + b'\n')
    return rd()

rd()
cmd({"execute": "qmp_capabilities"})

def inp(ev):
    cmd({"execute": "input-send-event", "arguments": {"events": ev}})
    time.sleep(0.1)

pos = [1018, 1014]
def moveto(x, y):
    while pos != [x, y]:
        dx = max(-120, min(120, x - pos[0]))
        dy = max(-120, min(120, y - pos[1]))
        ev = []
        if dx: ev.append({"type": "rel", "data": {"axis": "x", "value": dx}})
        if dy: ev.append({"type": "rel", "data": {"axis": "y", "value": dy}})
        inp(ev)
        pos[0] += dx; pos[1] += dy
    time.sleep(0.25)

def clickat(x, y):
    moveto(x, y)
    inp([{"type": "btn", "data": {"down": True, "button": "left"}}])
    time.sleep(0.12)
    inp([{"type": "btn", "data": {"down": False, "button": "left"}}])
    time.sleep(0.55)

# open music, then play
clickat(1105, 2004)
time.sleep(1.5)
cmd({"execute": "screendump", "arguments": {"filename": out + "/r-01-before-play.ppm"}})
time.sleep(0.5)
clickat(476, 468)   # play
time.sleep(3.0)
cmd({"execute": "screendump", "arguments": {"filename": out + "/r-02-after-play.ppm"}})
time.sleep(0.5)
cmd({"execute": "screendump", "arguments": {"filename": out + "/r-03-later.ppm"}})
PYEOF

for f in "$OUT"/r-0*.ppm; do
  [ -f "$f" ] || continue
  convert "$f" "${f%.ppm}.png"
  rm -f "$f"
done
cp -f "$OUT"/r-0*.png "/mnt/c/Klemens/Coding/Aspect OS/docs/screenshots/" 2>/dev/null || true
kill "$(cat "$OUT/qemu.pid")" 2>/dev/null || true
echo done
