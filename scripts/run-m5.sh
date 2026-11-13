#!/bin/bash
# ASPECT OS - M5 test: real file system (PCI + AHCI + ASPFS).
# QMP injection: read the Files app, run LS/CAT in the terminal, write a
# file, reboot via system_reset, and prove the file survived.
set -e
cd "/mnt/c/Klemens/Coding/Aspect OS"
OUT=/root/aspect-qemu
mkdir -p "$OUT"

if [ -f "$OUT/qemu.pid" ]; then
  kill "$(cat "$OUT/qemu.pid")" 2>/dev/null || true
  sleep 1
fi
rm -f "$OUT/qmp.sock" "$OUT"/m5-*.ppm "$OUT"/m5-*.png "$OUT/serial.log"

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

def key(q):
    s.sendall(b'{"execute":"send-key","arguments":{"keys":[{"type":"qcode","data":"' + q.encode() + b'"}]}}\n')
    rd()
    time.sleep(0.22)

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

QMAP = {' ': 'spc', '/': 'slash', '.': 'dot'}

def typeline(text):
    for ch in text:
        key(QMAP.get(ch, ch))
    key("ret")
    time.sleep(0.6)

# 1. boot desktop: Files lists the real ASPFS files
dump("m5-01-desktop")

# 1b. raise the Files window and select a file (content loads from disk)
moveto(300, 79); click()
moveto(150, 150); click()  # first file row (WELCOME.TXT)
time.sleep(0.8)
dump("m5-08-files")
moveto(620, 119); click()  # exposed part of the terminal title bar
time.sleep(0.4)

# 2. focus the terminal (title bar at 136,104 -> click away from traffic lights)
moveto(500, 119); click()
time.sleep(0.6)

# 3. LS
typeline("ls")
dump("m5-02-ls")

# 4. CAT an existing file
typeline("cat system/welcome.txt")
dump("m5-03-cat")

# 5. WRITE a new file
typeline("write guest/note.txt hello from m5")
dump("m5-04-write")

# 6. read it back
typeline("cat guest/note.txt")
dump("m5-05-catnew")

# 7. DF
typeline("df")
dump("m5-06-df")

# 8. reboot: the write must survive (image persists, system_reset reboots)
s.sendall(b'{"execute":"system_reset"}\n')
rd()
time.sleep(18)
# system_reset puts the guest cursor back at screen center
pos = [1018, 1014]
moveto(500, 119); click()
time.sleep(0.8)
click()  # make sure the terminal is raised even if the first click raced boot
time.sleep(0.5)
typeline("cat guest/note.txt")
dump("m5-07-persist")
PYEOF

for f in "$OUT"/m5-*.ppm; do
  [ -e "$f" ] || continue
  convert "$f" "${f%.ppm}.png"
  rm -f "$f"
done
mkdir -p /mnt/c/Klemens/Coding/Aspect\ OS/docs/screenshots
cp -f "$OUT"/m5-*.png /mnt/c/Klemens/Coding/Aspect\ OS/docs/screenshots/ 2>/dev/null || true
echo "--- serial.log ---"
cat "$OUT/serial.log" 2>/dev/null || echo "(no serial.log)"
