# ASPECT OS - build pipeline (Rust, no foreign crates)
#
#   kernel/main.rs  + modules  ->  build/kernel.elf  ->  build/kernel.bin
#   boot/main.rs               ->  build/BOOTX64.EFI   (kernel embedded via include_bytes!)
#   build-iso                  ->  build/aspect-os.iso
#
# usage (in WSL):  bash scripts/build.sh [target]
BUILDDIR := build
RUSTC    := /root/.cargo/bin/rustc

KERNEL_SRCS := kernel/main.rs kernel/gui.rs kernel/logo.rs kernel/wm.rs \
               kernel/apps.rs \
               kernel/serial.rs kernel/bootinfo.rs kernel/font.rs \
               kernel/gdt.rs kernel/idt.rs kernel/io.rs \
               kernel/pic.rs kernel/pit.rs \
               kernel/ps2.rs kernel/keyboard.rs kernel/mouse.rs \
               kernel/pci.rs kernel/ahci.rs kernel/fs.rs \
               kernel/e1000.rs kernel/net.rs \
               kernel/pmm.rs kernel/kernel.ld

all: $(BUILDDIR)/aspect-os.iso $(BUILDDIR)/aspect-data.img

# --- kernel: freestanding 64-bit ELF, then flat binary ---
$(BUILDDIR)/kernel.elf: $(KERNEL_SRCS) | $(BUILDDIR)
	$(RUSTC) --edition 2021 --target x86_64-unknown-none -O -C panic=abort \
	  -C relocation-model=static -C code-model=small \
	  -C link-arg=-T -C link-arg=kernel/kernel.ld \
	  kernel/main.rs -o $@

$(BUILDDIR)/kernel.bin: $(BUILDDIR)/kernel.elf
	objcopy -O binary $< $@

# --- bootloader: UEFI PE; the kernel is embedded, so build kernel.bin first ---
$(BUILDDIR)/BOOTX64.EFI: boot/main.rs $(BUILDDIR)/kernel.bin | $(BUILDDIR)
	$(RUSTC) --edition 2021 --target x86_64-unknown-uefi -O -C panic=abort \
	  -C relocation-model=static \
	  -C link-args="/ENTRY:efi_main /SUBSYSTEM:EFI_APPLICATION" \
	  boot/main.rs -o $@

# --- bootable ISO (UEFI El Torito, FAT image via mtools) ---
$(BUILDDIR)/aspect-os.iso: $(BUILDDIR)/BOOTX64.EFI
	bash scripts/build-iso.sh

# --- data disk: mkfs (host Rust) -> build/aspect-data.img ---
$(BUILDDIR)/mkfs: tools/mkfs.rs | $(BUILDDIR)
	$(RUSTC) --edition 2021 -O tools/mkfs.rs -o $@

$(BUILDDIR)/aspect-data.img: $(BUILDDIR)/mkfs
	$(BUILDDIR)/mkfs $@ 16

$(BUILDDIR):
	mkdir -p $(BUILDDIR)

# --- test: QEMU + OVMF, screendumps into docs/screenshots/ ---
test: all
	bash scripts/run-qemu.sh

clean:
	rm -rf $(BUILDDIR)

.PHONY: all test clean
