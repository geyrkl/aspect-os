#!/bin/bash
# ASPECT OS - bootfaehige UEFI-ISO bauen (eigener Bootloader, El Torito EFI)
# Wird vom Makefile aufgerufen, nachdem BOOTX64.EFI + kernel.bin gebaut wurden.
set -e
cd "$(dirname "$0")/.."

IMG=build/efi.img
rm -f "$IMG" build/aspect-os.iso
# 8 MB FAT-Image (unser ESP)
mformat -i "$IMG" -C -T 16384 -h 64 -s 32
mmd   -i "$IMG" ::/EFI ::/EFI/BOOT
mcopy -i "$IMG" build/BOOTX64.EFI ::/EFI/BOOT/BOOTX64.EFI
mcopy -i "$IMG" build/kernel.bin ::/KERNEL.SYS

# UEFI-El-Torito-Eintrag (nur UEFI, kein BIOS-Boot); Kernel steckt in BOOTX64.EFI
xorriso -as mkisofs -o build/aspect-os.iso -V "ASPECT_OS" \
  -eltorito-platform 0xEF -eltorito-boot efi.img -no-emul-boot \
  -c boot.cat build/efi.img

ls -la build/aspect-os.iso build/BOOTX64.EFI build/kernel.bin
echo "ISO_OK"
