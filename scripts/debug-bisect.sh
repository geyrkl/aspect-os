#!/bin/bash
# ASPECT OS - bisect session: isolate when corruption starts.
# Steps: baseline dump, click Terminal title, type into terminal,
# open Notes + SAVE (disk write), dump after each step.
# NOTE: keyboard input goes through HMP `sendkey` (human-monitor-command);
# QMP input-send-event kbd events do not reach the PS/2 controller.
set -e
cd "/mnt/c/Klemens/Coding/Aspect OS"
OUT=/root/aspect-qemu
mkdir -p "$OUT"

if [ -f "$OUT/qemu.pid" ]; then
  kill "$(cat "$OUT/qemu.pid")" 2>/dev/null || true
  sleep 1
fi
rm -f "$OUT/qmp.sock" "$OUT"/b-*.ppm "$OUT"/b-*.png "$OUT/serial.log"

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
    time.sleep(0.5)

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
    time.sleep(0.25)

def click():
    inp(b'[{"type":"btn","data":{"down":true,"button":"left"}}]')
    time.sleep(0.12)
    inp(b'[{"type":"btn","data":{"down":false,"button":"left"}}]')
    time.sleep(0.55)

def clickat(x, y):
    moveto(x, y)
    click()

def key(name):
    s.sendall(b'{"execute":"human-monitor-command","arguments":{"command-line":"sendkey '
              + name.encode() + b'"}}\n')
    rd()
    time.sleep(0.06)

KEYNAMES = {' ': 'spc', '_': 'shift-minus', '\n': 'ret'}
def typestr(t):
    for ch in t:
        key(KEYNAMES.get(ch, ch))

DOCK_Y = 2004
NOTES_X = 1051

# 1. baseline
dump("b-01-baseline")

# 2. click Terminal title bar (the m8-07 anomaly trigger)
clickat(600, 119)
dump("b-02-term-raise")

# 3. type into the terminal: click content first, then type + enter
clickat(400, 250)
typestr("help\n")
time.sleep(0.5)
dump("b-03-typed")

# 4. open Notes via dock and SAVE (disk write) with no typing.
#    Notes opens at k=3: window (216,184,560x380), content (218,214);
#    SAVE button content (w-78..w-16, y 8..32) -> abs (730,235)
clickat(NOTES_X, DOCK_Y)
time.sleep(1.5)
clickat(730, 235)   # SAVE button
time.sleep(0.5)
dump("b-04-notes-saved")

# 5. wait, dump again: does anything drift/corrupt over time?
time.sleep(3)
dump("b-05-later")

print(open(out + "/serial.log").read()[-300:])
PYEOF

for f in "$OUT"/b-*.ppm; do
  [ -f "$f" ] || continue
  convert "$f" "${f%.ppm}.png"
  rm -f "$f"
done
cp -f "$OUT"/b-*.png "/mnt/c/Klemens/Coding/Aspect OS/docs/screenshots/" 2>/dev/null || true
echo "--- dumps ---"
ls "$OUT"/b-*.png 2>/dev/null || true
