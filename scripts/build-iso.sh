#!/bin/bash
# ASPECT OS - build a bootable UEFI ISO (own bootloader, El Torito EFI)
# Called by the Makefile after BOOTX64.EFI + kernel.bin have been built.
set -e
cd "$(dirname "$0")/.."

IMG=build/efi.img
rm -f "$IMG" build/aspect-os.iso
# 8 MB FAT image (our ESP)
mformat -i "$IMG" -C -T 16384 -h 64 -s 32
mmd   -i "$IMG" ::/EFI ::/EFI/BOOT
mcopy -i "$IMG" build/BOOTX64.EFI ::/EFI/BOOT/BOOTX64.EFI
mcopy -i "$IMG" build/kernel.bin ::/KERNEL.SYS

# UEFI El Torito entry (UEFI only, no BIOS boot); kernel is embedded in BOOTX64.EFI
xorriso -as mkisofs -o build/aspect-os.iso -V "ASPECT_OS" \
  -eltorito-platform 0xEF -eltorito-boot efi.img -no-emul-boot \
  -c boot.cat build/efi.img

ls -la build/aspect-os.iso build/BOOTX64.EFI build/kernel.bin
echo "ISO_OK"
