#!/bin/bash
# one-off AHCI trace run (debugging the zero-transfer issue)
cd "/mnt/c/Klemens/Coding/Aspect OS"
kill "$(cat /root/aspect-qemu/qemu.pid)" 2>/dev/null; sleep 1
CODE=/usr/share/OVMF/OVMF_CODE_4M.fd
cp /usr/share/OVMF/OVMF_VARS_4M.fd /root/aspect-qemu/VARS2.fd
qemu-system-x86_64 \
  -enable-kvm -m 2048 -cpu host -smp 2 \
  -drive if=pflash,format=raw,readonly=on,file="$CODE" \
  -drive if=pflash,format=raw,file=/root/aspect-qemu/VARS2.fd \
  -drive file=build/aspect-os.iso,media=cdrom,readonly=on \
  -device ich9-ahci,id=ahci0 \
  -drive file=build/aspect-data.img,format=raw,if=none,id=aspfs \
  -device ide-hd,drive=aspfs,bus=ahci0.0 \
  -nic user,model=e1000 \
  -serial file:/root/aspect-qemu/serial2.log \
  -display none -daemonize \
  -d 'trace:ahci*' -D /root/aspect-qemu/ahci.trace \
  -pidfile /root/aspect-qemu/qemu2.pid
sleep 17
kill "$(cat /root/aspect-qemu/qemu2.pid)" 2>/dev/null
grep -a -E 'populate_sglist|prepare_buf|start_dma|dma_cb|handle_cmd|handle_reg|cmd_done' /root/aspect-qemu/ahci.trace | tail -40
echo ------
grep -a 'fs:' /root/aspect-qemu/serial2.log
