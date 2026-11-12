# ASPECT OS - Build-Pipeline (Rust, keine Fremd-Crates)
#
#   kernel/main.rs  + Module   ->  build/kernel.elf  ->  build/kernel.bin
#   boot/main.rs               ->  build/BOOTX64.EFI   (Kernel via include_bytes! eingebettet)
#   build-iso                  ->  build/aspect-os.iso
#
# Aufruf (im WSL):  bash scripts/build.sh [target]
BUILDDIR := build
RUSTC    := /root/.cargo/bin/rustc

KERNEL_SRCS := kernel/main.rs kernel/gui.rs kernel/logo.rs \
               kernel/serial.rs kernel/bootinfo.rs kernel/kernel.ld

all: $(BUILDDIR)/aspect-os.iso

# --- Kernel: freistehender 64-bit ELF, dann flat binary ---
$(BUILDDIR)/kernel.elf: $(KERNEL_SRCS) | $(BUILDDIR)
	$(RUSTC) --edition 2021 --target x86_64-unknown-none -O -C panic=abort \
	  -C relocation-model=static -C code-model=small \
	  -C link-arg=-T -C link-arg=kernel/kernel.ld \
	  kernel/main.rs -o $@

$(BUILDDIR)/kernel.bin: $(BUILDDIR)/kernel.elf
	objcopy -O binary $< $@

# --- Bootloader: UEFI-PE; der Kernel ist eingebettet, also kernel.bin zuerst ---
$(BUILDDIR)/BOOTX64.EFI: boot/main.rs $(BUILDDIR)/kernel.bin | $(BUILDDIR)
	$(RUSTC) --edition 2021 --target x86_64-unknown-uefi -O -C panic=abort \
	  -C relocation-model=static \
	  -C link-args="/ENTRY:efi_main /SUBSYSTEM:EFI_APPLICATION" \
	  boot/main.rs -o $@

# --- Bootfaehige ISO (UEFI El Torito, FAT-Image via mtools) ---
$(BUILDDIR)/aspect-os.iso: $(BUILDDIR)/BOOTX64.EFI
	bash scripts/build-iso.sh

$(BUILDDIR):
	mkdir -p $(BUILDDIR)

# --- Test: QEMU + OVMF, Screendump nach docs/screenshots/ ---
test: all
	bash scripts/run-qemu.sh

clean:
	rm -rf $(BUILDDIR)

.PHONY: all test clean
