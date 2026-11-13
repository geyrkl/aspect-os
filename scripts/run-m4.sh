#!/bin/bash
# ASPECT OS - M4 test: compositor + apps (QEMU + OVMF + QMP injection)
set -e
cd "/mnt/c/Klemens/Coding/Aspect OS"
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
  -nic user,model=e1000 \
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

def dump(name):
    s.sendall(b'{"execute":"screendump","arguments":{"filename":"' + out.encode() + b'/' + name.encode() + b'.ppm"}}\n')
    rd()
    time.sleep(0.4)

def inp(events):
    s.sendall(b'{"execute":"input-send-event","arguments":{"events":' + events + b'}}\n')
    rd()
    time.sleep(0.15)

pos = [1018, 1014]  # cursor starts at screen center

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

# dock icon centers (2048x2048 screen): x = 889 + i*54, y = 2004
DOCK = lambda i: (889 + i * 54, 2004)

# 1. boot screen: three windows (files, terminal, calc)
dump("01-desktop")

# 2. open PAINT from the dock
moveto(*DOCK(2)); click()
time.sleep(1.2)
dump("02-paint")

# 3. paint strokes: press in the canvas, drag around, release
moveto(500, 500); hold()
moveto(700, 620); moveto(900, 500); moveto(1000, 660); release()
time.sleep(1.0)
dump("03-paintdraw")

# 4. raise + drag the terminal by its title bar, then type HELP
moveto(400, 119); hold()
moveto(600, 300); release()
time.sleep(0.6)
for q in ("h", "e", "l", "p"):
    key(q)
key("ret")
time.sleep(0.8)
dump("04-terminal")

# 5. open SETTINGS from the dock and switch to SUNSET (row 1)
moveto(*DOCK(4)); click()
time.sleep(1.0)
moveto(300, 372); click()   # theme row 1 of the settings window
time.sleep(1.0)
dump("05-settings")

# 6. open BROWSER from the dock and navigate to MANUAL via the URL bar
moveto(*DOCK(0)); click()
time.sleep(1.0)
for q in ("m", "a", "n", "u", "a", "l"):
    key(q)
key("ret")
time.sleep(0.8)
dump("06-browser")

# 7. close the front window (browser) via its red traffic light
#    browser window: k=5 -> x=296, y=264 ; red light at x+18, y+15
moveto(314, 279); click()
time.sleep(1.0)
dump("07-close")

dump("08-final")
PYEOF

for f in "$OUT"/*.ppm; do
  [ -f "$f" ] || continue
  convert "$f" "${f%.ppm}.png"
  rm -f "$f"
done
mkdir -p /mnt/c/Klemens/Coding/Aspect\ OS/docs/screenshots
cp -f "$OUT"/*.png /mnt/c/Klemens/Coding/Aspect\ OS/docs/screenshots/ 2>/dev/null || true
echo "--- serial.log ---"
cat "$OUT/serial.log" 2>/dev/null || echo "(no serial.log)"
