#!/bin/bash
# ASPECT OS - M8 motion test: dock magnification + launch bounce + running
# dots, music player with equalizer, notes with disk persistence (reboot),
# traffic-light hover glyphs, close ghost, menu slide-in.
# Dumps land in docs/screenshots/ as m8-*.png.
#
# Keyboard input goes through HMP `sendkey` (human-monitor-command):
# QMP input-send-event kbd events do NOT reach the PS/2 controller (they
# only produce a stray 0xFA ack in the kernel trace).
#
# Dumps taken while an animation runs (EQ, ghosts, menu slide) can catch
# the compositor mid-frame and tear; each animated step therefore dumps
# twice (the *.b copy is a second try ~0.3 s later).
#
# Geometry (screen 2048x2048):
#   dock: dx0=801, resting slot centers x = 835+i*54, y = 2004
#   boot windows: FILES (96,64) TERMINAL (136,104) CALC (176,144)
#   new windows k=3: (216,184) k=4: (256,224); content = (x+2, y+30)
#   MUSIC  (k=3): play button abs (476,468)
#   NOTES  (k=4): SAVE button abs (760,272); editor takes keyboard focus
#   TERMINAL title visible band: x 556..696, y 104..134 -> hover (600,119)
#   red traffic light of TERMINAL: (154,119)
#   FILE menu title with CALCULATOR app menu: (197,14)
set -e
cd "/mnt/c/Klemens/Coding/Aspect OS"
OUT=/root/aspect-qemu
mkdir -p "$OUT"

if [ -f "$OUT/qemu.pid" ]; then
  kill "$(cat "$OUT/qemu.pid")" 2>/dev/null || true
  sleep 1
fi
rm -f "$OUT/qmp.sock" "$OUT"/m8-*.ppm "$OUT"/m8-*.png "$OUT/serial.log"

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
MUSIC_X = 1105   # resting center of dock slot 5 (music)
NOTES_X = 1051   # resting center of dock slot 4 (notes)

# 0. input probe: does a lone QMP kbd event reach the kernel at all?
cmd({"execute": "input-send-event", "arguments": {"events": [
    {"type": "kbd", "data": {"down": True, "key": {"qcode": "a"}}}]}})
time.sleep(0.1)
cmd({"execute": "input-send-event", "arguments": {"events": [
    {"type": "kbd", "data": {"down": False, "key": {"qcode": "a"}}}]}})
time.sleep(0.3)

# 1. hover the dock: icons magnify around the cursor, running dots below
#    the open apps (FILES/TERMINAL/CALC at boot)
moveto(MUSIC_X, DOCK_Y - 120)   # approach from above
moveto(MUSIC_X, DOCK_Y)
time.sleep(0.6)
dump("m8-01-dockmag")

# 2. click MUSIC: bounce hop + window pops in (mid-animation)
clickat(MUSIC_X, DOCK_Y)
time.sleep(0.05)
dump("m8-02-launch")

# 3. music paused -> press PLAY -> equalizer animates
time.sleep(1.2)
dump("m8-03-music-paused")
clickat(476, 468)                # play button
time.sleep(1.4)
dump("m8-04-music-playing")
time.sleep(0.3)
dump("m8-04b-music-playing2")
time.sleep(0.9)
dump("m8-05-music-eq2")          # bars moved on
time.sleep(0.3)
dump("m8-05b-music-eq2b")

# 4. open NOTES, type, SAVE (writes NOTE1.TXT to disk); the equalizer keeps
#    animating in the background, so dumps here can tear
clickat(NOTES_X, DOCK_Y)
time.sleep(1.2)
typestr("hello from m8")
clickat(760, 272)                # SAVE button
time.sleep(0.5)
dump("m8-06-notes-saved")
time.sleep(0.3)
dump("m8-06b-notes-saved2")

# 5. hover the TERMINAL title bar: x / minus / zoom glyphs on the lights
clickat(600, 119)
time.sleep(0.4)
dump("m8-07-lightglyphs")
time.sleep(0.3)
dump("m8-07b-lightglyphs2")

# 6. reboot: type `reboot` in the terminal (8042 reset), wait, NOTES shows
#    the saved note from disk
typestr("reboot\n")
print("[reboot] waiting 22s ...")
time.sleep(22)
pos = [1018, 1014]   # the OS re-inits its cursor at screen center after reset
clickat(NOTES_X, DOCK_Y)
time.sleep(1.2)
dump("m8-08-notes-persist")

# 7. raise TERMINAL, close it: shrinking ghost mid-animation
clickat(600, 119)
time.sleep(0.4)
clickat(154, 119)                # red traffic light
time.sleep(0.08)
dump("m8-09-close-ghost")
time.sleep(0.25)
dump("m8-09b-close-ghost2")

# 8. FILE menu mid slide-in and settled
time.sleep(0.6)
clickat(197, 14)
dump("m8-10-menu-slide")
time.sleep(0.3)
dump("m8-10b-menu-slide2")
time.sleep(0.5)
dump("m8-11-menu-open")

# serial tail for context
print(open(out + "/serial.log").read()[-300:])
PYEOF

for f in "$OUT"/m8-*.ppm; do
  [ -f "$f" ] || continue
  convert "$f" "${f%.ppm}.png"
  rm -f "$f"
done
mkdir -p /mnt/c/Klemens/Coding/Aspect\ OS/docs/screenshots
cp -f "$OUT"/m8-*.png /mnt/c/Klemens/Coding/Aspect\ OS/docs/screenshots/ 2>/dev/null || true
echo "--- dumps copied ---"
ls "$OUT"/m8-*.png 2>/dev/null || true
