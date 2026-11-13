// ASPECT OS - own PCI configuration-space scanner (ports 0xCF8/0xCFC).
// Finds the AHCI storage controller (class 01/06) and hands its BAR5
// memory base to the AHCI driver. No firmware tables, no external code.
#![allow(unknown_lints, static_mut_refs, dead_code)]

use crate::io;
use core::sync::atomic::{AtomicU64, AtomicBool, Ordering};

static AHCI_BAR: AtomicU64 = AtomicU64::new(0);
static AHCI_FOUND: AtomicBool = AtomicBool::new(false);

#[inline]
fn addr(bus: u32, dev: u32, fn_: u32, reg: u32) -> u32 {
    0x8000_0000 | (bus << 16) | (dev << 11) | (fn_ << 8) | ((reg & 0xFC) as u32)
}

fn read32(bus: u32, dev: u32, fn_: u32, reg: u32) -> u32 {
    io::outl(0xCF8, addr(bus, dev, fn_, reg));
    io::inl(0xCFC)
}

fn vendor(bus: u32, dev: u32, fn_: u32) -> u16 {
    (read32(bus, dev, fn_, 0) & 0xFFFF) as u16
}

/// scan the PCI bus for an AHCI controller; enables its memory space and
/// bus mastering, then stores BAR5. Uses physical addresses directly (the
/// kernel runs identity-mapped like the GOP framebuffer).
pub unsafe fn init() {
    'bus: for bus in 0..16u32 {
        for dev in 0..32u32 {
            if vendor(bus, dev, 0) == 0xFFFF {
                continue; // no device at function 0
            }
            for fn_ in 0..8u32 {
                if vendor(bus, dev, fn_) == 0xFFFF {
                    continue;
                }
                let r8 = read32(bus, dev, fn_, 8);
                let class = (r8 >> 24) & 0xFF;
                let subcl = (r8 >> 16) & 0xFF;
                if class == 0x01 && subcl == 0x06 {
                    // AHCI found: BAR5 holds the AHCI base (ABAR)
                    let bar = read32(bus, dev, fn_, 0x24);
                    // enable memory space (bit 1) + bus master (bit 2)
                    let cmd = read32(bus, dev, fn_, 4);
                    io::outl(0xCF8, addr(bus, dev, fn_, 4));
                    io::outl(0xCFC, cmd | 0x6);
                    // 64-bit BARs: read the upper half too (usually 0)
                    let mut base = (bar & 0xFFFF_FFF0) as u64;
                    if bar & 0x4 != 0 {
                        let hi = read32(bus, dev, fn_, 0x28) as u64;
                        base |= hi << 32;
                    }
                    AHCI_BAR.store(base, Ordering::Relaxed);
                    AHCI_FOUND.store(true, Ordering::Relaxed);
                    break 'bus;
                }
            }
        }
    }
}

/// physical AHCI base address (0 = no controller found)
pub fn bar() -> u64 {
    AHCI_BAR.load(Ordering::Relaxed)
}

pub fn found() -> bool {
    AHCI_FOUND.load(Ordering::Relaxed)
}
