# ASPECT OS

Ein **komplett von Grund auf** gebautes Betriebssystem in **Rust** — kein Linux,
kein GRUB, keine Fremd-Crates, keine vorgefertigten Themes. Eigener
UEFI-Bootloader, eigener 64-bit-Kernel, eigene GUI mit macOS-Look.

## Projektstruktur

```
.
├── Makefile                  # Build-Pipeline (rustc -> EFI/ELF -> ISO)
├── boot/
│   └── main.rs               # UEFI-Bootloader (eigene UEFI-Header, Handover)
├── kernel/
│   ├── main.rs               # Kernel-Einstieg (kmain, .bss-Nullung, hlt-Loop)
│   ├── gui.rs                # Framebuffer-GUI: Gradient, Menueleiste, Dock
│   ├── logo.rs               # "ASPECT OS"-Wordmark als Vektor-Strokes
│   ├── serial.rs             # COM1-Debug-Ausgabe
│   ├── bootinfo.rs           # geteilte Struktur Bootloader -> Kernel
│   └── kernel.ld             # Linker-Skript (Kernel @ 0x100000, .text.first)
├── scripts/
│   ├── build.sh              # WSL-Build-Einstieg (Toolchain-Check + make)
│   ├── build-iso.sh          # FAT-ESP (mtools) + UEFI-El-Torito-ISO (xorriso)
│   ├── run-qemu.sh           # QEMU/OVMF-Test, Screendump via QMP
│   ├── run-test.sh           # Wraper fuer run-qemu.sh aus Windows
│   ├── start-os.sh/.ps1      # OS live in einem QEMU-Fenster starten
│   └── wsl-setup.sh          # Toolchain-Installation (Rust, mtools, OVMF, ...)
├── docs/
│   └── screenshots/          # QEMU-Screendumps
├── reference/                # Design-Referenz (nur Farben/Motive, gitignored)
└── LICENSE                   # MIT
```

## Architektur

| Komponente | Beschreibung |
|---|---|
| **Bootloader** `boot/main.rs` | Eigene `repr(C)`-UEFI-Header (kein GNU-EFI), GOP-Videomodus (hoechste Aufloesung), Kernel wird **eingebettet** (`include_bytes!`) und nach `0x100000` kopiert, eigener Stack, Memory-Map, `ExitBootServices` mit Retry, Handover via `asm!` (`rdi` = BootInfo) |
| **Kernel** `kernel/` | `.bss`-Nullung ueber Linker-Symbole, Stufe-Logging via COM1, `hlt`-Loop |
| **GUI** `kernel/gui.rs` | 4-Farb-Bilinear-Gradient (LiquidGlass-Farben), Alpha-Blending, Rundrechtecke (Scanline + `isqrt`), Bresenham-Linien mit Pinsel, SDF-Strokes, Menueleiste (Monogramm/Wifi/Batterie), Dock mit 6 Icons |
| **Build** | `rustc --target x86_64-unknown-uefi` -> `BOOTX64.EFI` (rust-lld); `rustc --target x86_64-unknown-none` -> ELF -> `objcopy` -> flat binary; ISO via mtools + xorriso |

## Build & Test (WSL Ubuntu-24.04)

```powershell
# Build (Toolchain wird bei Bedarf installiert)
Copy-Item "scripts\build.sh" "\\wsl.localhost\Ubuntu-24.04\root\build-aspect.sh" -Force
wsl -d Ubuntu-24.04 -u root -- bash -c "bash /root/build-aspect.sh"

# QEMU-Test mit Screendump (landet in docs/screenshots/)
Copy-Item "scripts\run-test.sh" "\\wsl.localhost\Ubuntu-24.04\root\run-test.sh" -Force
wsl -d Ubuntu-24.04 -u root -- bash -c "bash /root/run-test.sh"
```

## OS live starten

```powershell
powershell -ExecutionPolicy Bypass -File "scripts\start-os.ps1"
```

## Roadmap

- [x] **M1** Boot + Gradient-Desktop mit Wordmark, Menueleiste, Dock (Rust)
- [ ] **M2** eigene GDT/IDT, Exception-Handler, PIT-Timer -> Uhr in der Menueleiste
- [ ] **M3** eigene Treiber: PS/2-Maus + Tastatur, eigener Physical-Allocator
- [ ] **M4** Fenster-Compositor: Verschieben, Schliessen, Blur-Optik
- [ ] **M5** eigene Bitmap-Schrift (ASCII), Uhr-/Menue-Texte, Wordmark-Feinschliff
- [ ] **M6** Boot auf echter Hardware (USB-Stick)

## Lizenz

MIT — siehe [LICENSE](LICENSE).
