#!/bin/bash
# ASPECT OS - menubar menu test: opens every macOS-style menu, picks items,
# switches a theme, closes a window through the app menu. Screendumps land
# in docs/screenshots/ as dm-*.png.
set -e
cd "/mnt/c/Klemens/Coding/Aspect OS"
OUT=/root/aspect-qemu
mkdir -p "$OUT"

if [ -f "$OUT/qemu.pid" ]; then
  kill "$(cat "$OUT/qemu.pid")" 2>/dev/null || true
  sleep 1
fi
rm -f "$OUT/qmp.sock" "$OUT"/dm-*.ppm "$OUT"/dm-*.png "$OUT/serial.log"

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

# menubar geometry (scale-2 font, computed like menu_rect). IMPORTANT:
# the OS boots with FILES/TERMINAL/CALC windows open, so the app menu reads
# "CALCULATOR" first - close those windows to get the clean "FINDER" bar.
# FINDER bar:  monogram (6..30) | FINDER (38..108) | FILE (126..172)
#              VIEW (190..236)  | WINDOW (254..324) | HELP (342..388)
# FILES bar:    FILE (114..160)  | VIEW (178..224)  | WINDOW (242..312)
# TERMINAL bar: FILE (150..196)  | VIEW (214..260)  | WINDOW (278..348)
# Item rows: text at y=33+r*20, row band 31+r*20..51+r*20, panel x = title-6.

# 0. close the three boot windows via their red traffic lights
clickat(194, 159)   # CALCULATOR  (176,144)
clickat(154, 119)   # TERMINAL    (136,104)
clickat(114, 79)    # FILES       (96,64)
time.sleep(0.5)

# 1. monogram menu: ABOUT THIS OS | SETTINGS
clickat(18, 14)
dump("dm-01-appmenu")

# 2. FILE menu with its items (close the monogram menu first - a click on
#    a hover-switched title toggles it closed, like macOS)
clickat(1600, 900)
clickat(149, 14)
dump("dm-02-filemenu")

# 3. pick OPEN FILES, then OPEN TERMINAL through the same menu
clickat(160, 41)    # row 0: OPEN FILES
time.sleep(1.0)
clickat(137, 14)    # FILE title moved: app menu now reads FILES
clickat(148, 61)    # row 1: OPEN TERMINAL
time.sleep(1.0)

# 4. WINDOW menu lists both windows, checkmark on the focused one
clickat(277, 14)
dump("dm-03-windowmenu")

# 5. close it, then the VIEW menu with the theme checkmarks
#    (TERMINAL app menu: FILE 150..196, VIEW 214..260, WINDOW 278..348)
clickat(1600, 900)
clickat(237, 14)
dump("dm-04-viewmenu")

# 6. pick SUNSET - theme applies, menu closes (panel x=208, row 1 = SUNSET)
clickat(250, 61)
time.sleep(1.0)
dump("dm-05-sunset")

# 7. app menu of the focused TERMINAL: CLOSE WINDOW closes it
clickat(85, 14)
dump("dm-06-app-terminal")
clickat(131, 41)
time.sleep(1.0)
dump("dm-07-closed")

# 8. monogram menu again: ABOUT THIS OS opens the About window
clickat(18, 14)
clickat(18, 41)     # row 0: ABOUT THIS OS
time.sleep(1.0)
dump("dm-08-about")

# serial tail for context
print(open(out + "/serial.log").read()[-300:])
PYEOF

for f in "$OUT"/dm-*.ppm; do
  [ -f "$f" ] || continue
  convert "$f" "${f%.ppm}.png"
  rm -f "$f"
done
mkdir -p /mnt/c/Klemens/Coding/Aspect\ OS/docs/screenshots
cp -f "$OUT"/dm-*.png /mnt/c/Klemens/Coding/Aspect\ OS/docs/screenshots/ 2>/dev/null || true
echo "--- dumps copied ---"
ls "$OUT"/dm-*.png 2>/dev/null || true
