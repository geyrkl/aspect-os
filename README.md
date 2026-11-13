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
│   ├── gui.rs                # GUI: draw targets, clipping, themes, text, dock, cursor
│   ├── wm.rs                 # window manager + compositor (drag, focus, queues)
│   ├── apps.rs               # bundled apps: files, terminal, calc, paint, settings, browser
│   ├── font.rs               # 4x7 clock digits + 5x7 ASCII font (95 glyphs)
│   ├── gdt.rs                # own GDT (ring-0 code/data) + CS/SS reload
│   ├── idt.rs                # own IDT, exception stubs, IRQ0/1/12 dispatcher
│   ├── pic.rs                # 8259A PIC remap (vectors 32..47)
│   ├── pit.rs                # PIT channel 0 as system tick (1000 Hz)
│   ├── ps2.rs                # 8042 controller init (keyboard + mouse)
│   ├── keyboard.rs           # scancode set 2 -> key codes (shift map, debounce)
│   ├── mouse.rs              # 3-byte packets -> cursor deltas + buttons
│   ├── pmm.rs                # bitmap physical memory allocator (4 KiB frames)
│   ├── io.rs                 # port I/O (outb/inb)
│   ├── logo.rs               # "ASPECT OS" wordmark as vector strokes
│   ├── serial.rs             # COM1 debug output
│   ├── bootinfo.rs           # shared structure bootloader -> kernel
│   └── kernel.ld             # linker script (kernel @ 0x2000000, .text.first)
├── scripts/
│   ├── build.sh              # WSL build entry point (toolchain check + make)
│   ├── build-iso.sh          # FAT ESP (mtools) + UEFI El-Torito ISO (xorriso)
│   ├── run-qemu.sh           # QEMU/OVMF test, screendumps via QMP
│   ├── run-m4.sh             # M4 acceptance run: apps, drag, keys, themes, browser
│   ├── debug-m4.sh           # short M4 debug run (strokes, drag, keys)
│   ├── run-test.sh           # Windows wrapper for run-qemu.sh
│   ├── start-os.sh/.ps1      # boot the OS live in a QEMU window (fallback)
│   ├── start-os-vnc.sh       # headless QEMU serving VNC on 127.0.0.1:5901
│   ├── vnc-viewer.ps1        # own RFB client: viewer window on Windows
│   └── wsl-setup.sh          # toolchain install (Rust, mtools, OVMF, ...)
├── docs/
│   └── screenshots/          # QEMU screendumps
├── reference/                # design reference (colors/motifs only, gitignored)
└── LICENSE                   # MIT
```

## Architecture

| Component | Description |
|---|---|
| **Bootloader** `boot/main.rs` | Own `repr(C)` UEFI headers (no GNU-EFI), GOP video mode (highest resolution), kernel **embedded** (`include_bytes!`) and copied to `0x2000000` (32 MiB span reserved: the image includes ~19 MiB of `.bss` buffers, and OVMF reserves everything below ~24 MiB), own stack, memory map copied into `BootInfo` before `ExitBootServices` (with retry), handover via `asm!` (`rdi` = BootInfo) |
| **Kernel** `kernel/` | `.bss` zeroing (u64 bulk + byte head/tail), staged logging via COM1, event loop (`sti`/`hlt`): drains mouse/key queues, repaints on demand, ticks the menubar clock once per second |
| **GUI** `kernel/gui.rs` | Draw targets (framebuffer / 2048x2048 background buffer / app canvas) with clipping, 4 themes (AURORA/SUNSET/OCEAN/FOREST) with bilinear gradients, alpha blending, rounded rects (scanline + `isqrt`), Bresenham brush lines, SDF strokes, menubar (monogram/wifi/battery/clock), dock with 6 app icons, 5x7 ASCII bitmap font (`font.rs`), pre-rendered background blitted in u64 bulk, arrow cursor drawn last (no save/restore) |
| **Interrupts (M2)** | `gdt.rs` own GDT with far-return CS reload, `idt.rs` 32 exception stubs (global_asm, own register save) + IRQ0/1/12 dispatcher (IRQ1/12 push events into the WM queues), `pic.rs` 8259A remap to 32..47, `pit.rs` timer @ 1000 Hz |
| **Input + memory (M3)** | `ps2.rs` 8042 init (translation off, scancode set 2, timeouts), `keyboard.rs` keymap + shift map + 20 ms make-code debounce, `mouse.rs` 3-byte packet parser (`Option` events only on completed packets), `pmm.rs` bitmap frame allocator fed by the UEFI memory map from the bootloader |
| **Window manager (M4)** `wm.rs` | Window list with z-order by index (max 8), ring-buffer input queues (IRQ producer / main-loop consumer), raise on click, title-bar drag, red traffic light closes, dock launches apps, content clicks/moves/keys routed to the focused app, compositor pass: background -> windows (shadow, chrome, clipped content) -> cursor |
| **Apps (M4)** `apps.rs` | Files (static VFS: 3 folders x 3 text files, tabs + detail pane), Terminal (13x38 text grid: help/clear/echo/ver/uptime/mem/apps/about), Calculator (i64 integer math, ERROR on /0), Paint (960x560 canvas, 8-color palette, brush strokes), Settings (4 themes, instant desktop regen), Browser (offline pages: HOME/ABOUT/MANUAL/404, URL bar), About (via the menubar monogram) |
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

One command from Windows - build, boot, and the OS appears in its own window:

```powershell
powershell -ExecutionPolicy Bypass -File "scripts\start-os.ps1"
```

- The ISO is rebuilt automatically when sources are newer (skip with `-NoBuild`).
- QEMU runs **headless inside WSL** and serves VNC on `127.0.0.1:5901`.
- The viewer is our own RFB client (`scripts\vnc-viewer.ps1`, C# via
  `Add-Type`, no external dependencies): a scaled window showing the guest
  framebuffer. Mouse and keyboard work reliably there, because VNC pointer
  events are absolute and WSLg window capture is bypassed entirely.
- `F11` toggles fullscreen, `Alt+F4` quits; closing the window stops QEMU.
- `-Fullscreen` starts borderless maximized.

Fallback for quick boot checks only (QEMU window via WSLg, mouse capture may
not work under WSLg):

```powershell
powershell -ExecutionPolicy Bypass -File "scripts\start-os.ps1" -Window
```

## Roadmap

- [x] **M1** Boot + gradient desktop with wordmark, menubar, dock (Rust)
- [x] **M2** Own GDT/IDT, exception handlers, PIT timer -> clock in the menubar
- [x] **M3** Own drivers: PS/2 mouse + keyboard, own physical allocator
- [x] **M4** Window compositor + apps: drag/focus/close, file manager, terminal, calculator, paint, settings (themes), offline browser, about
- [ ] **M5** Real file system (ATA/AHCI + own FS format)
- [ ] **M6** Network stack, then real hardware (USB stick)

## License

MIT — see [LICENSE](LICENSE).
