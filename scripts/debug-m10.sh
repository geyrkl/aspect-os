#!/bin/bash
# ASPECT OS - M10 usability test: minimize + restore (yellow light / dock),
# resize grip, zoom toggle (green light), terminal history (arrow keys),
# WINDOW menu with MINIMIZE/ZOOM rows.
# Dumps land in docs/screenshots/ as m10-*.png.
#
# Keyboard input goes through HMP `sendkey` (QMP kbd events do NOT reach
# the PS/2 controller). Dumps during animations can tear - animated steps
# dump twice (the *.b copy ~0.3 s later).
#
# Geometry (screen 2048x2048):
#   dock: resting slot centers x = 835+i*54, y = 2004
#     FILES=889 PAINT=943 TERMINAL=997 NOTES=1051 MUSIC=1105
#     SETTINGS=1159 CALC=1213
#   boot windows: FILES (96,64,460x320) TERMINAL (136,104,560x380)
#     CALC (176,144,300x420); content = (x+2, y+30)
#   TERMINAL lights y=119: red 154, yellow 170, green 186
#   CALC lights y=159: red 194, yellow 210, green 226
#   CALC grip (bottom-right, initial): (467,555)
#   zoomed window: green light at (50,51)
#   WINDOW menu title (app=TERMINAL): WINDOW 278..348 -> click (310,14)
set -e
cd "/mnt/c/Klemens/Coding/Aspect OS"
OUT=/root/aspect-qemu
mkdir -p "$OUT"

if [ -f "$OUT/qemu.pid" ]; then
  kill "$(cat "$OUT/qemu.pid")" 2>/dev/null || true
  sleep 1
fi
rm -f "$OUT/qmp.sock" "$OUT"/m10-*.ppm "$OUT"/m10-*.png "$OUT/serial.log"

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

def cmd(o):
    s.sendall(json.dumps(o).encode() + b'\n')
    return rd()

def dump(name):
    s.sendall(b'{"execute":"screendump","arguments":{"filename":"' + out.encode() + b'/' + name.encode() + b'.ppm"}}\n')
    rd()
    time.sleep(0.5)

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
    time.sleep(0.25)

def click():
    inp(b'[{"type":"btn","data":{"down":true,"button":"left"}}]')
    time.sleep(0.12)
    inp(b'[{"type":"btn","data":{"down":false,"button":"left"}}]')
    time.sleep(0.55)

def clickat(x, y):
    moveto(x, y)
    click()

def dragto(x, y):
    # press, move while held, release (resize grip / window drag)
    inp(b'[{"type":"btn","data":{"down":true,"button":"left"}}]')
    time.sleep(0.2)
    moveto(x, y)
    time.sleep(0.2)
    inp(b'[{"type":"btn","data":{"down":false,"button":"left"}}]')
    time.sleep(0.6)

def dragat(x0, y0, x1, y1):
    moveto(x0, y0)
    dragto(x1, y1)

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
TERM_DOCK = 997   # dock slot 3 = terminal

# 1. minimize the TERMINAL via the yellow light: it shrinks away (ghost),
#    the CALC window underneath becomes focused (colored lights)
clickat(170, 119)                 # TERMINAL yellow light
time.sleep(0.15)
dump("m10-01-minimize")           # mid shrink
time.sleep(0.5)
dump("m10-01b-minimize2")         # settled: terminal gone, CALC focused

# 2. restore it from the dock: running dot still there, pop-in animation
clickat(TERM_DOCK, DOCK_Y)
time.sleep(0.15)
dump("m10-02-restore")
time.sleep(0.6)
dump("m10-02b-restore2")

# 3. resize the CALC window from its grip: +93 wide, +145 tall
clickat(326, 159)                 # raise CALC (title bar)
dragat(467, 555, 560, 700)
time.sleep(0.4)
dump("m10-03-resize")

# 4. zoom CALC via the green light: fills the work area (menubar..dock)
clickat(226, 159)
time.sleep(0.8)
dump("m10-04-zoom")
time.sleep(0.3)
dump("m10-04b-zoom2")

# 5. un-zoom via the green light again (zoomed geometry: light at 50,51)
clickat(50, 51)
time.sleep(0.8)
dump("m10-05-unzoom")

# 6. terminal command history: run two commands, then arrow-up twice
#    recalls them (ls first) and Enter executes
clickat(600, 119)                 # raise TERMINAL
typestr("ls\n")
time.sleep(0.4)
typestr("ver\n")
time.sleep(0.4)
key("up"); key("up"); key("ret")  # history: recall "ls", execute
time.sleep(0.5)
dump("m10-06-history")

# 7. WINDOW menu: window list + separator + MINIMIZE + ZOOM rows
#    menubar geometry (advance 6*sc, sc=2, app=TERMINAL tw=94):
#    app 38..132, FILE 150..196, VIEW 214..260, WINDOW 278..348 (hit 272..354)
clickat(310, 14)
time.sleep(0.4)
dump("m10-07-window-menu")
time.sleep(0.3)
dump("m10-07b-window-menu2")
# pick MINIMIZE (row 4 of 6): panel x=272..446, rows y=31+r*20, row 4 = 111..130
# move down inside the panel FIRST - a horizontal move along the bar would
# hover-switch to HELP (its extended hit rect starts at x=360)
moveto(310, 110)
moveto(360, 121)
click()
time.sleep(0.6)
dump("m10-08-minimized")          # terminal gone again, menubar = FINDER

# 8. restore once more from the dock (final state sanity)
clickat(TERM_DOCK, DOCK_Y)
time.sleep(1.0)
dump("m10-09-final")

# serial tail for context
print(open(out + "/serial.log").read()[-300:])
PYEOF

for f in "$OUT"/m10-*.ppm; do
  [ -f "$f" ] || continue
  convert "$f" "${f%.ppm}.png"
  rm -f "$f"
done
mkdir -p /mnt/c/Klemens/Coding/Aspect\ OS/docs/screenshots
cp -f "$OUT"/m10-*.png /mnt/c/Klemens/Coding/Aspect\ OS/docs/screenshots/ 2>/dev/null || true
echo "--- dumps copied ---"
ls "$OUT"/m10-*.png 2>/dev/null || true
