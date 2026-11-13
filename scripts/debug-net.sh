#!/bin/bash
# ASPECT OS - network debug run: QEMU packet capture + e1000 trace.
# Boots the OS, types IP and PING in the terminal, captures all traffic
# with a QEMU filter-dump, then prints the packets and the e1000 trace.
set -e
cd "/mnt/c/Klemens/Coding/Aspect OS"
OUT=/root/aspect-qemu
mkdir -p "$OUT"

if [ -f "$OUT/qemu.pid" ]; then
  kill "$(cat "$OUT/qemu.pid")" 2>/dev/null || true
  sleep 1
fi
rm -f "$OUT/qmp.sock" "$OUT/net.pcap" "$OUT/trace.log" "$OUT/serial.log"

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
  -netdev user,id=n0 \
  -device e1000,netdev=n0 \
  -object filter-dump,id=nd,netdev=n0,file="$OUT/net.pcap" \
  -d 'trace:e1000*' \
  -D "$OUT/trace.log" \
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

def inp(events):
    s.sendall(b'{"execute":"input-send-event","arguments":{"events":' + events + b'}}\n')
    rd()
    time.sleep(0.15)

def key(q):
    s.sendall(b'{"execute":"send-key","arguments":{"keys":[{"type":"qcode","data":"' + q.encode() + b'"}]}}\n')
    rd()
    time.sleep(0.22)

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

# focus the terminal, run ip, then ping
moveto(500, 119); click()
time.sleep(0.6)
for ch in "ip":
    key(ch)
key("ret")
time.sleep(1.0)
for ch in "ping":
    key(ch)
key("ret")
print("[QMP] ping sent, waiting 10 s ...")
time.sleep(10)
PYEOF

kill "$(cat "$OUT/qemu.pid")" 2>/dev/null || true
sleep 1

echo "--- serial.log (tail) ---"
tail -8 "$OUT/serial.log"

echo "--- packets ---"
python3 - "$OUT/net.pcap" <<'PYEOF'
import struct, sys
data = open(sys.argv[1], 'rb').read()
magic = data[:4]
if magic == b'\xd4\xc3\xb2\xa1':
    endian = '<'
elif magic == b'\xa1\xb2\xc3\xd4':
    endian = '>'
else:
    print("not a pcap:", magic.hex()); sys.exit(1)
off = 24
n = 0
while off + 16 <= len(data):
    ts, tus, cap, orig = struct.unpack(endian + 'IIII', data[off:off+16])
    off += 16
    pkt = data[off:off+cap]; off += cap
    n += 1
    if len(pkt) >= 14:
        dst = ':'.join(f'{b:02x}' for b in pkt[0:6])
        src = ':'.join(f'{b:02x}' for b in pkt[6:12])
        et = (pkt[12] << 8) | pkt[13]
        body = pkt[14:]
        info = ''
        if et == 0x0806 and len(body) >= 28:
            oper = (body[6] << 8) | body[7]
            spa = '.'.join(str(b) for b in body[14:18])
            tpa = '.'.join(str(b) for b in body[24:28])
            info = f'ARP {"req" if oper == 1 else "rep"} who-has {tpa} tell {spa}'
        elif et == 0x0800 and len(body) >= 20:
            ihl = (body[0] & 0xf) * 4
            proto = body[9]
            src_ip = '.'.join(str(b) for b in body[12:16])
            dst_ip = '.'.join(str(b) for b in body[16:20])
            tot = (body[2] << 8) | body[3]
            hdr = body[:ihl]
            csum = (body[10] << 8) | body[11]
            s = 0
            h = bytearray(hdr); h[10:12] = b'\x00\x00'
            for i in range(0, ihl, 2):
                s += (h[i] << 8) | h[i+1]
            while s > 0xffff: s = (s & 0xffff) + (s >> 16)
            good = ((~s) & 0xffff) == csum
            if proto == 1:
                icmp = body[ihl:]
                it = icmp[0] if icmp else -1
                icmp_info = 'echo-req' if it == 8 else ('echo-rep' if it == 0 else f'type {it}')
                ic = 0
                c = bytearray(icmp); c[2:4] = b'\x00\x00'
                for i in range(0, len(c) - 1, 2):
                    ic += (c[i] << 8) | c[i+1]
                if len(c) % 2: ic += c[-1] << 8
                while ic > 0xffff: ic = (ic & 0xffff) + (ic >> 16)
                ic_good = ((~ic) & 0xffff) == ((icmp[2] << 8) | icmp[3])
                info = f'ICMP {icmp_info} ip-csum-{"ok" if good else "BAD"} icmp-csum-{"ok" if ic_good else "BAD"}'
            else:
                info = f'proto {proto} ip-csum-{"ok" if good else "BAD"}'
            print(f'#{n} IPv4 {src_ip} -> {dst_ip} len {tot}: {info}')
            off2 = off - cap + 14
            continue
        elif et == 0x0806:
            pass
        print(f'#{n} eth {src} -> {dst} etype {et:#06x}')
    else:
        print(f'#{n} short frame {len(pkt)} bytes')
PYEOF

echo "--- e1000 trace (guest-visible ops, tail) ---"
grep -E 'e1000_(xmit|tx_|rx)' "$OUT/trace.log" | head -40
