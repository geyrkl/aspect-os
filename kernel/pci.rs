// ASPECT OS - own PCI configuration-space scanner (ports 0xCF8/0xCFC).
// Finds the storage (AHCI, class 01/06) and network (ethernet, class 02/00)
// controllers and hands their MMIO bases to the drivers. No firmware
// tables, no external code.
#![allow(unknown_lints, static_mut_refs, dead_code)]

use crate::io;
use core::sync::atomic::{AtomicU64, AtomicBool, Ordering};

static AHCI_BAR: AtomicU64 = AtomicU64::new(0);
static AHCI_FOUND: AtomicBool = AtomicBool::new(false);
static NIC_BAR: AtomicU64 = AtomicU64::new(0);
static NIC_FOUND: AtomicBool = AtomicBool::new(false);

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

/// claim a device: enable memory space (bit 1) + bus mastering (bit 2),
/// return the 64-bit base address of `bar_reg`
fn claim(bus: u32, dev: u32, fn_: u32, bar_reg: u32) -> u64 {
    let bar = read32(bus, dev, fn_, bar_reg);
    let cmd = read32(bus, dev, fn_, 4);
    io::outl(0xCF8, addr(bus, dev, fn_, 4));
    io::outl(0xCFC, cmd | 0x6);
    let mut base = (bar & 0xFFFF_FFF0) as u64;
    if bar & 0x4 != 0 {
        // 64-bit BAR: read the upper half too (usually 0)
        let hi = read32(bus, dev, fn_, bar_reg + 4) as u64;
        base |= hi << 32;
    }
    base
}

/// scan the PCI bus for the devices we drive. Uses physical addresses
/// directly (the kernel runs identity-mapped like the GOP framebuffer).
pub unsafe fn init() {
    for bus in 0..16u32 {
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
                if class == 0x01 && subcl == 0x06 && !AHCI_FOUND.load(Ordering::Relaxed) {
                    // AHCI: BAR5 holds the AHCI base (ABAR)
                    let base = claim(bus, dev, fn_, 0x24);
                    AHCI_BAR.store(base, Ordering::Relaxed);
                    AHCI_FOUND.store(true, Ordering::Relaxed);
                } else if class == 0x02 && subcl == 0x00 && !NIC_FOUND.load(Ordering::Relaxed) {
                    // ethernet: BAR0 holds the MMIO register base
                    let base = claim(bus, dev, fn_, 0x10);
                    NIC_BAR.store(base, Ordering::Relaxed);
                    NIC_FOUND.store(true, Ordering::Relaxed);
                }
            }
        }
    }
}

/// physical AHCI base address (0 = no controller found)
pub fn bar() -> u64 {
    AHCI_BAR.load(Ordering::Relaxed)
}

/// physical ethernet MMIO base address (0 = no controller found)
pub fn nic_bar() -> u64 {
    NIC_BAR.load(Ordering::Relaxed)
}

pub fn found() -> bool {
    AHCI_FOUND.load(Ordering::Relaxed)
}
