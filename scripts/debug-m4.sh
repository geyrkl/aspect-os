#!/bin/bash
# ASPECT OS - short debug run: paint hold+drag, keys
set -e
cd "/mnt/c/Klemens/Coding/Aspect OS"
OUT=/root/aspect-qemu
mkdir -p "$OUT"

if [ -f "$OUT/qemu.pid" ]; then
  kill "$(cat "$OUT/qemu.pid")" 2>/dev/null || true
  sleep 1
fi
rm -f "$OUT/qmp.sock" "$OUT"/*.ppm "$OUT/serial.log"

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
rd()
s.sendall(b'{"execute":"qmp_capabilities"}\n'); rd()

def inp(events):
    s.sendall(b'{"execute":"input-send-event","arguments":{"events":' + events + b'}}\n')
    rd()
    time.sleep(0.15)

pos = [1018, 1014]

def moveto(x, y):
    while pos != [x, y]:
        dx = max(-120, min(120, x - pos[0]))
        dy = max(-120, min(120, y - pos[1]))
        if dx:
            inp(b'[{"type":"rel","data":{"axis":"x","value":' + str(dx).encode() + b'}}]')
        if dy:
            inp(b'[{"type":"rel","data":{"axis":"y","value":' + str(dy).encode() + b'}}]')
        pos[0] += dx; pos[1] += dy
    time.sleep(0.2)

def click():
    inp(b'[{"type":"btn","data":{"down":true,"button":"left"}}]')
    time.sleep(0.1)
    inp(b'[{"type":"btn","data":{"down":false,"button":"left"}}]')
    time.sleep(0.5)

def hold():
    inp(b'[{"type":"btn","data":{"down":true,"button":"left"}}]')
    time.sleep(0.1)

def release():
    inp(b'[{"type":"btn","data":{"down":false,"button":"left"}}]')
    time.sleep(0.3)

def key(q):
    s.sendall(b'{"execute":"send-key","arguments":{"keys":[{"type":"qcode","data":"' + q.encode() + b'"}]}}\n')
    rd()
    time.sleep(0.25)

def dump(name):
    s.sendall(b'{"execute":"screendump","arguments":{"filename":"' + out.encode() + b'/' + name.encode() + b'.ppm"}}\n')
    rd()
    time.sleep(0.4)

# open paint
moveto(997, 2004); click()
time.sleep(1.0)
# press inside canvas and drag (paint window is front, canvas shows strokes)
moveto(500, 500); hold()
moveto(700, 620); release()
time.sleep(0.5)
dump("d1-stroke")

# drag the FILES window (k=0: x=96, y=64) by its title bar
moveto(400, 119); hold()
moveto(600, 300); release()
time.sleep(0.8)
dump("d2-drag")

# raise the terminal by clicking inside its content area (dragged to
# (336, 285): content (338, 315) - (894, 645)), then type HELP
moveto(700, 500); click()
time.sleep(0.5)
for q in ("h", "e", "l", "p"):
    key(q)
key("ret")
time.sleep(0.5)
dump("d3-keys")
PYEOF
for f in "$OUT"/d*.ppm; do
  [ -f "$f" ] || continue
  convert "$f" "${f%.ppm}.png"
  rm -f "$f"
done
cp -f "$OUT"/d*.png /mnt/c/Klemens/Coding/Aspect\ OS/build/ 2>/dev/null || true
echo "--- serial ---"
grep -a -v 'm dx=0 dy=0' "$OUT/serial.log" | tail -130
