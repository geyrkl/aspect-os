#!/bin/bash
# ASPECT OS - minimal input diagnostics: does QMP kbd / HMP sendkey reach the
# kernel? where does a rel-mouse click really land?
set -e
cd "/mnt/c/Klemens/Coding/Aspect OS"
OUT=/root/aspect-qemu
mkdir -p "$OUT"

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
echo "[QEMU] PID: $(cat "$OUT/qemu.pid")"
sleep 15

python3 - "$OUT" <<'PYEOF'
import socket, sys, time
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
import json

def cmd(o):
    s.sendall(json.dumps(o).encode() + b'\n')
    return rd()

rd()
cmd({"execute": "qmp_capabilities"})

def tail(label):
    time.sleep(0.6)
    data = open(out + "/serial.log").read()
    idx = data.rfind("compositor running")
    print(f"--- {label} ---")
    print(data[idx:] if idx >= 0 else data[-400:])

# 1. QMP kbd qcode 'h' (scancode set2 make 0x33)
cmd({"execute": "input-send-event", "arguments": {"events": [
    {"type": "kbd", "data": {"down": True, "key": {"qcode": "h"}}}]}})
time.sleep(0.1)
cmd({"execute": "input-send-event", "arguments": {"events": [
    {"type": "kbd", "data": {"down": False, "key": {"qcode": "h"}}}]}})
tail("after QMP kbd h")

# 2. HMP sendkey h
cmd({"execute": "human-monitor-command", "arguments": {"command-line": "sendkey h"}})
tail("after HMP sendkey h")

# 3. mouse: rel-move to (600,119) from center (1018,1014), click
def inp(ev):
    cmd({"execute": "input-send-event", "arguments": {"events": ev}})
    time.sleep(0.1)

pos = [1018, 1014]
while pos != [600, 119]:
    dx = max(-120, min(120, 600 - pos[0]))
    dy = max(-120, min(120, 119 - pos[1]))
    ev = []
    if dx:
        ev.append({"type": "rel", "data": {"axis": "x", "value": dx}})
    if dy:
        ev.append({"type": "rel", "data": {"axis": "y", "value": dy}})
    inp(ev)
    pos[0] += dx; pos[1] += dy
time.sleep(0.3)
inp([{"type": "btn", "data": {"down": True, "button": "left"}}])
time.sleep(0.12)
inp([{"type": "btn", "data": {"down": False, "button": "left"}}])
tail("after click at (600,119)")
PYEOF
echo "--- done ---"
