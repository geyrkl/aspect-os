#!/bin/bash
# ASPECT OS - M6 test: network stack (PCI + e1000 + ARP/IP/ICMP).
# QMP injection: IP command shows MAC/address, PING gets real echo
# replies from QEMU's user-network gateway (10.0.2.2).
set -e
cd "/mnt/c/Klemens/Coding/Aspect OS"
OUT=/root/aspect-qemu
mkdir -p "$OUT"

if [ -f "$OUT/qemu.pid" ]; then
  kill "$(cat "$OUT/qemu.pid")" 2>/dev/null || true
  sleep 1
fi
rm -f "$OUT/qmp.sock" "$OUT"/m6-*.ppm "$OUT"/m6-*.png "$OUT/serial.log"

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

def typeline(text):
    for ch in text:
        key({' ': 'spc', '/': 'slash', '.': 'dot'}.get(ch, ch))
    key("ret")
    time.sleep(0.6)

# 1. boot desktop
dump("m6-01-desktop")

# 2. focus the terminal
moveto(500, 119); click()
time.sleep(0.6)

# 3. IP: show MAC + address
typeline("ip")
dump("m6-02-ip")

# 4. PING: 4 echo requests to the slirp gateway (the command blocks the
#    event loop for up to ~2 s, so wait long before the screendump)
typeline("ping")
time.sleep(4.5)
dump("m6-03-ping")

# 5. help shows the new commands
typeline("help")
time.sleep(1.0)
dump("m6-04-help")
PYEOF

for f in "$OUT"/m6-*.ppm; do
  [ -e "$f" ] || continue
  convert "$f" "${f%.ppm}.png"
  rm -f "$f"
done
mkdir -p /mnt/c/Klemens/Coding/Aspect\ OS/docs/screenshots
cp -f "$OUT"/m6-*.png /mnt/c/Klemens/Coding/Aspect\ OS/docs/screenshots/ 2>/dev/null || true
echo "--- serial.log ---"
cat "$OUT/serial.log" 2>/dev/null || echo "(no serial.log)"
