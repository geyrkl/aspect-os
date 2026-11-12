# ASPECT OS

A **from-scratch** operating system written entirely in **Rust** — no Linux,
no GRUB, no foreign crates, no pre-made themes. Own UEFI bootloader, own
64-bit kernel, own macOS-style GUI.

## Project layout

```
.
├── Makefile                  # build pipeline (rustc -> EFI/ELF -> ISO)
├── boot/
│   └── main.rs               # UEFI bootloader (own UEFI headers, handover)
├── kernel/
│   ├── main.rs               # kernel entry (kmain, .bss zeroing, event loop)
│   ├── gui.rs                # framebuffer GUI: gradient, menubar, dock, clock
│   ├── font.rs               # bitmap digits (4x7) for the menubar clock
│   ├── gdt.rs                # own GDT (ring-0 code/data) + CS/SS reload
│   ├── idt.rs                # own IDT, exception stubs, IRQ0/1/12 dispatcher
│   ├── pic.rs                # 8259A PIC remap (vectors 32..47)
│   ├── pit.rs                # PIT channel 0 as system tick (1000 Hz)
│   ├── ps2.rs                # 8042 controller init (keyboard + mouse)
│   ├── keyboard.rs           # scancode set 2 -> key events
│   ├── mouse.rs              # 3-byte packets -> cursor deltas + buttons
│   ├── pmm.rs                # bitmap physical memory allocator (4 KiB frames)
│   ├── io.rs                 # port I/O (outb/inb)
│   ├── logo.rs               # "ASPECT OS" wordmark as vector strokes
│   ├── serial.rs             # COM1 debug output
│   ├── bootinfo.rs           # shared structure bootloader -> kernel
│   └── kernel.ld             # linker script (kernel @ 0x100000, .text.first)
├── scripts/
│   ├── build.sh              # WSL build entry point (toolchain check + make)
│   ├── build-iso.sh          # FAT ESP (mtools) + UEFI El-Torito ISO (xorriso)
│   ├── run-qemu.sh           # QEMU/OVMF test, screendumps via QMP
│   ├── run-test.sh           # Windows wrapper for run-qemu.sh
│   ├── start-os.sh/.ps1      # boot the OS live in a QEMU window
│   └── wsl-setup.sh          # toolchain install (Rust, mtools, OVMF, ...)
├── docs/
│   └── screenshots/          # QEMU screendumps
├── reference/                # design reference (colors/motifs only, gitignored)
└── LICENSE                   # MIT
```

## Architecture

| Component | Description |
|---|---|
| **Bootloader** `boot/main.rs` | Own `repr(C)` UEFI headers (no GNU-EFI), GOP video mode (highest resolution), kernel **embedded** (`include_bytes!`) and copied to `0x100000`, own stack, memory map copied into `BootInfo` before `ExitBootServices` (with retry), handover via `asm!` (`rdi` = BootInfo) |
| **Kernel** `kernel/` | `.bss` zeroing via linker symbols, staged logging via COM1, event loop (`sti`/`hlt`) |
| **GUI** `kernel/gui.rs` | 4-color bilinear gradient (LiquidGlass palette), alpha blending, rounded rects (scanline + `isqrt`), Bresenham lines with brush, SDF strokes, menubar (monogram/wifi/battery/clock), dock with 6 icons, mouse cursor with background save/restore |
| **Interrupts (M2)** | `gdt.rs` own GDT with far-return CS reload, `idt.rs` 32 exception stubs (global_asm, own register save) + IRQ0/1/12 dispatcher, `pic.rs` 8259A remap to 32..47, `pit.rs` timer @ 1000 Hz, clock in the menubar via own 4x7 bitmap digits |
| **Input + memory (M3)** | `ps2.rs` 8042 init (translation off, scancode set 2, timeouts), `keyboard.rs` keymap, `mouse.rs` packet parser, `pmm.rs` bitmap frame allocator fed by the UEFI memory map from the bootloader |
| **Build** | `rustc --target x86_64-unknown-uefi` -> `BOOTX64.EFI` (rust-lld); `rustc --target x86_64-unknown-none` -> ELF -> `objcopy` -> flat binary; ISO via mtools + xorriso |

## Build & Test (WSL Ubuntu-24.04)

```powershell
# Build (toolchain gets installed if missing)
Copy-Item "scripts\build.sh" "\\wsl.localhost\Ubuntu-24.04\root\build-aspect.sh" -Force
wsl -d Ubuntu-24.04 -u root -- bash -c "bash /root/build-aspect.sh"

# QEMU test with screendumps (land in docs/screenshots/)
Copy-Item "scripts\build-test.sh" "\\wsl.localhost\Ubuntu-24.04\root\build-test.sh" -Force
Copy-Item "scripts\run-test.sh" "\\wsl.localhost\Ubuntu-24.04\root\run-test.sh" -Force
wsl -d Ubuntu-24.04 -u root -- bash -c "bash /root/build-test.sh"
```

## Run the OS live

```powershell
powershell -ExecutionPolicy Bypass -File "scripts\start-os.ps1"
```

## Roadmap

- [x] **M1** Boot + gradient desktop with wordmark, menubar, dock (Rust)
- [x] **M2** Own GDT/IDT, exception handlers, PIT timer -> clock in the menubar
- [ ] **M3** Own drivers: PS/2 mouse + keyboard, own physical allocator
- [ ] **M4** Window compositor: move, close, blur effects
- [ ] **M5** Own bitmap font (ASCII), clock/menu text, wordmark polish
- [ ] **M6** Boot on real hardware (USB stick)

## License

MIT — see [LICENSE](LICENSE).
