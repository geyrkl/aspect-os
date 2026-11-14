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
│   ├── wm.rs                 # window manager + compositor (drag, focus, queues, menubar menus)
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
│   ├── pci.rs                # own PCI config-space scan (finds AHCI + e1000 NIC)
│   ├── ahci.rs               # own AHCI driver (DMA EXT read/write, polling)
│   ├── e1000.rs              # own Intel e1000 NIC driver (TX/RX rings, polling)
│   ├── net.rs                # own network stack: ARP, IPv4, ICMP (ping)
│   ├── fs.rs                 # ASPFS: own file system (superblock/bitmap/dirs)
│   ├── io.rs                 # port I/O (outb/inb/outl/inl)
│   ├── logo.rs               # "ASPECT OS" wordmark as vector strokes
│   ├── serial.rs             # COM1 debug output
│   ├── bootinfo.rs           # shared structure bootloader -> kernel
│   └── kernel.ld             # linker script (kernel @ 0x2000000, .text.first)
├── tools/
│   └── mkfs.rs               # host tool: creates the ASPFS data disk image
├── scripts/
│   ├── build.sh              # WSL build entry point (toolchain check + make)
│   ├── build-iso.sh          # FAT ESP (mtools) + UEFI El-Torito ISO (xorriso)
│   ├── run-qemu.sh           # QEMU/OVMF test, screendumps via QMP
│   ├── run-m4.sh             # M4 acceptance run: apps, drag, keys, themes, browser
│   ├── run-m5.sh             # M5 acceptance run: real FS, terminal disk access, reboot persistence
│   ├── run-m6.sh             # M6 acceptance run: IP info, ping the gateway, help
│   ├── debug-m4.sh           # short M4 debug run (strokes, drag, keys)
│   ├── debug-ahci.sh         # short AHCI trace run (QEMU trace:ahci* debugging)
│   ├── debug-net.sh          # short network debug run (QEMU filter-dump pcap analysis)
│   ├── debug-menu.sh         # menubar menu acceptance run: every dropdown, theme pick, window list
│   ├── debug-motion.sh       # M8 acceptance run: dock animations, music EQ, notes persistence (reboot)
│   ├── debug-m10.sh          # M10 acceptance run: minimize/restore, resize grip, zoom, terminal history, WINDOW menu
│   ├── debug-bisect.sh       # short bisect run: input/write steps with dumps after each
│   ├── debug-input.sh        # input diagnostics: QMP kbd vs HMP sendkey, click coordinates
│   ├── debug-play.sh         # repro: music play with per-frame window-list trace
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
| **GUI** `kernel/gui.rs` | Draw targets (framebuffer / 2048x2048 background buffer / app canvas) with clipping, 4 themes (AURORA/SUNSET/OCEAN/FOREST) with bilinear gradients, alpha blending, rounded rects (scanline + `isqrt`), Bresenham brush lines, SDF strokes, menubar (monogram/wifi/battery/clock), dock as a translucent glass pill with a hairline top edge, ambient wallpaper glow blobs tinted from the theme, bulk pixel-rect blits (`blit_rect`, used for the paint canvas), macOS-style soft drop shadows (3 layered translucent rounded rects), 5x7 ASCII bitmap font (`font.rs`), pre-rendered background blitted in u64 bulk, arrow cursor drawn last with a white outline (no save/restore) |
| **Interrupts (M2)** | `gdt.rs` own GDT with far-return CS reload, `idt.rs` 32 exception stubs (global_asm, own register save) + IRQ0/1/12 dispatcher (IRQ1/12 push events into the WM queues), `pic.rs` 8259A remap to 32..47, `pit.rs` timer @ 1000 Hz |
| **Input + memory (M3)** | `ps2.rs` 8042 init (translation off, scancode set 2, timeouts), `keyboard.rs` keymap + shift map + 20 ms make-code debounce, `mouse.rs` 3-byte packet parser (`Option` events only on completed packets), `pmm.rs` bitmap frame allocator fed by the UEFI memory map from the bootloader |
| **Window manager (M4)** `wm.rs` | Window list with z-order by index (max 8), ring-buffer input queues (IRQ producer / main-loop consumer), raise on click, title-bar drag, red traffic light closes, dock launches apps, content clicks/moves/keys routed to the focused app, macOS-style menubar menus (monogram/app/FILE/VIEW/WINDOW/HELP dropdowns with live hover switching, item highlighting, checkmarks, outside-click close, any key closes), compositor pass: background -> windows (shadow, chrome, clipped content) -> menus -> cursor |
| **Motion (M8)** `wm.rs` | Eased open/close pop (scale around the window center, 220 ms), shrinking close ghosts drawn over the live windows, dock icons magnify under the cursor and launch icons hop (bounce), running-indicator dots under open apps, traffic-light hover glyphs on the focused window only (x/minus/zoom), menubar menus slide in, dirty-flag compositor throttled to ~60 fps with app-driven animation requests (equalizer) |
| **Polish (M9)** `wm.rs`/`gui.rs` | Authentic macOS look: colored traffic lights on the focused window, gray ones on unfocused windows, hairline under the title bar, soft shadows fade in with the window pop, dropdown menus get a layered shadow + border, the dock floats **above** every window (macOS z-order), macOS accent blue (`0xFF3D6EB4`) across menus/selection/controls; idle path repaints only the menubar clock once per second instead of a full frame |
| **Usability (M10)** `wm.rs`/`keyboard.rs`/`apps.rs` | Live window management: bottom-right resize grip (clamped to per-app minimum sizes), yellow light **minimizes** to the dock (window hides, leaves a shrinking ghost), dock click / WINDOW-menu row restores it with a pop-in, green light + title double-click **zoom** to the work area (restoring the previous geometry on toggle), arrow keys (PS/2 E0-extended) reach the apps and drive a terminal command history (up/down, 8 entries, duplicate suppression), focus is the topmost *visible* window everywhere (minimized windows can't be focused or hit), WINDOW menu lists all windows with the focused one checked plus MINIMIZE/ZOOM rows |
| **Apps (M4)** `apps.rs` | Files (backed by ASPFS on disk: tabs + file list + detail pane loaded from disk), Terminal (13x38 text grid: help/clear/echo/ver/uptime/mem/apps/about + disk commands ls/cat/write/rm/df + network commands ip/ping + reboot via 8042 reset), Calculator (i64 integer math, ERROR on /0), Paint (960x560 canvas, 8-color palette, brush strokes), Settings (4 themes, instant desktop regen), Browser (offline pages: HOME/ABOUT/MANUAL/404, URL bar), About + Settings (via the monogram menu, manual page also via the HELP menu) |
| **Apps (M8)** `apps.rs` | Music (playlist NEON DRIVE/SUNSET GRID/DEEP OCEAN, play/pause/prev/next, seek bar, live equalizer bars while playing), Notes (sidebar note list + editor, SAVE writes NOTE1..NOTE6.TXT to disk, notes reload from disk on boot and survive reboots); the paint canvas is blitted as bulk rows instead of 537k per-pixel writes |
| **Disk (M5)** `pci.rs`/`ahci.rs` | Own PCI config-space scan (ports 0xCF8/0xCFC): finds the AHCI controller (class 0106), enables memory space + bus mastering, hands BAR5 to the driver. `ahci.rs`: HBA init (GHC.AE), first port with DET=3, command list + FIS receive + command table in frames from our own PMM, READ/WRITE DMA EXT (LBA48) with polling (no interrupts), DMA staging buffer in `.bss` |
| **File system (M5)** `fs.rs` | **ASPFS**, the own format: 4 KiB blocks, block 0 superblock (magic `ASPFS`), block bitmap (cached in memory), chained directory blocks with 128-byte entries (kind/name/start/count/size), files as contiguous extents. Mount/list/read/write/delete/free, all writes go back to disk through the AHCI driver — files survive reboots |
| **mkfs (M5)** `tools/mkfs.rs` | Host tool (plain Rust, no deps): formats `build/aspect-data.img` (16 MiB) with the ASPFS layout and the seed files (SYSTEM/USERS/GUEST folders); wired into `make all` |
| **Network (M6)** `pci.rs`/`e1000.rs`/`net.rs` | The PCI scan also finds the NIC (class 0200) and hands its BAR0 to the driver. `e1000.rs`: own Intel e1000 (8254x) driver — TX/RX descriptor rings in `.bss` (direct DMA addresses, identity-mapped kernel), EEPROM MAC read, register polling (no NIC interrupts), descriptor-done (DD) handshake. `net.rs`: own stack — ARP (request/reply + cache), IPv4 (header build, checksum, routing through the gateway for off-subnet targets), ICMP echo (reply to requests, ping with per-packet RTT). Terminal commands `IP` (MAC/IP/GW/packet counters) and `PING [X.Y.Z.W]` (4 echo requests, default: the gateway). Panic handler logs to COM1 and halts instead of hanging silently |
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
- A real data disk (`build/aspect-data.img`, ASPFS, built by `make all`) is
  attached via QEMU's ich9-ahci — files created in the OS survive reboots.
- Networking works out of the box: the VM gets an e1000 NIC on QEMU's
  user-mode network (guest `10.0.2.15`, gateway `10.0.2.2`). Open the
  Terminal and run `IP` or `PING [X.Y.Z.W]` (default target: the gateway).
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
- [x] **M5** Real file system: own PCI scan, own AHCI driver, own ASPFS format + mkfs tool; Files app and terminal read/write real, reboot-persistent files
- [x] **M6** Network stack (QEMU): own PCI NIC discovery, own e1000 driver, own ARP/IPv4/ICMP stack, `IP`/`PING` terminal commands; real hardware next (USB stick)
- [x] **M7** macOS-style menubar menus: monogram/app/FILE/VIEW/WINDOW/HELP dropdowns, live hover switching, theme picker with checkmarks, window list
- [x] **M8** macOS polish: window open/close pop + close ghost, dock magnification + launch bounce + running dots, traffic-light hover glyphs, animated music player (equalizer, playlist, seek bar), Notes app with disk persistence, per-frame compositor throttling, `REBOOT` terminal command (8042 reset)
- [x] **M9** authentic macOS look + robustness: soft drop shadows, gray traffic lights on unfocused windows, title-bar hairline, glass dock pill + wallpaper glows, cursor outline, dock z-order above windows, macOS accent blue, bulk-canvas blits, clock-only idle repaint (version 0.6 "AQUA")
- [x] **M10** usability & resilience: window resize grip with per-app minimum sizes, minimize to dock (yellow light, shrinking ghost) + restore from dock/WINDOW menu with pop-in, zoom toggle (green light + title double-click), arrow-key terminal command history (up/down), focus = topmost visible window (minimized windows skipped in focus/hit-tests), WINDOW menu with window list + MINIMIZE/ZOOM

## License

MIT — see [LICENSE](LICENSE).
