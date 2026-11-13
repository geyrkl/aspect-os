// ASPECT OS - own AHCI (SATA) host controller driver. No interrupts, no
// firmware: initialize the HBA memory registers, build a command list and
// command table in frames from our own physical allocator, then issue
// READ DMA EXT / WRITE DMA EXT commands (LBA48) and poll for completion.
#![allow(unknown_lints, static_mut_refs, dead_code)]

use crate::pmm;
use crate::serial;
use core::sync::atomic::{AtomicBool, Ordering};

const HBA_CAP: usize = 0x00; // host capabilities
const HBA_GHC: usize = 0x04; // global host control (AE = bit 31)
const PORT_OFF: usize = 0x100; // port registers start
const PORT_STRIDE: usize = 0x80;

// port register offsets
const P_CLB: usize = 0x00; // command list base
const P_CLBU: usize = 0x04; // command list base upper
const P_FB: usize = 0x08; // FIS receive base
const P_FBU: usize = 0x0C; // FIS receive base upper
const P_IS: usize = 0x10; // interrupt status (we poll, keep IE = 0)
const P_IE: usize = 0x14;
const P_CMD: usize = 0x18; // command+status (ST bit0, FRE bit4, CR bit15)
const P_TFD: usize = 0x20; // task file data (ERR bit8, BSY bit0, DRQ bit3)
const P_SSTS: usize = 0x28; // status (DET bits 0..3, IPM bits 8..12)
const P_CI: usize = 0x38; // command issue

const PXSLOTS: usize = 32; // command list slots
const CFL: u32 = 5; // host-to-device FIS length in dwords

static mut BAR: u64 = 0; // physical AHCI base (identity-mapped)
static mut PORT: usize = usize::MAX; // port register base (BAR + offset)
static mut CLB: u64 = 0; // command list (physical)
static mut FB: u64 = 0; // FIS receive area (physical)
static mut CTB: u64 = 0; // command table (physical)
static READY: AtomicBool = AtomicBool::new(false);

// DMA staging buffer for block transfers (in .bss, identity-mapped)
const MAX_CHUNK: usize = 32 * 1024; // one PRDT entry covers up to 32 KiB
static mut DMA: [u8; MAX_CHUNK] = [0; MAX_CHUNK];

#[inline]
unsafe fn rd(off: usize) -> u32 {
    ((BAR + off as u64) as *mut u32).read_volatile()
}

#[inline]
unsafe fn wr(off: usize, val: u32) {
    ((BAR + off as u64) as *mut u32).write_volatile(val);
}

#[inline]
unsafe fn prd(poff: usize, off: usize) -> u32 {
    rd(PORT_OFF + poff * PORT_STRIDE + off)
}

#[inline]
unsafe fn pwr(poff: usize, off: usize, val: u32) {
    wr(PORT_OFF + poff * PORT_STRIDE + off, val);
}

#[inline]
fn timeout(mut n: u32) -> bool {
    while n > 0 {
        core::hint::spin_loop();
        n -= 1;
    }
    false // convenience: callers poll until their own condition breaks
}

/// stop the command engine (ST clear, wait CR clear)
unsafe fn port_stop(p: usize) {
    let cmd = prd(p, P_CMD);
    pwr(p, P_CMD, cmd & !1);
    let mut n = 1_000_000u32;
    while n > 0 && prd(p, P_CMD) & (1 << 15) != 0 {
        core::hint::spin_loop();
        n -= 1;
    }
}

pub unsafe fn init(bar: u64) -> bool {
    if bar == 0 {
        serial::log("ahci: no controller found\n");
        return false;
    }
    BAR = bar;
    // enable AHCI mode (GHC.AE)
    wr(HBA_GHC, rd(HBA_GHC) | (1 << 31));
    // clear the (unused) interrupt enable of all ports
    for p in 0..32usize {
        pwr(p, P_IE, 0);
    }

    // find the first port with a device present and communicating
    let mut p = usize::MAX;
    for i in 0..32usize {
        let ssts = prd(i, P_SSTS);
        if ssts & 0xF == 3 {
            p = i;
            break;
        }
    }
    if p == usize::MAX {
        serial::log("ahci: no device present\n");
        return false;
    }
    PORT = PORT_OFF + p * PORT_STRIDE;

    // engine off, then install our own command list + FIS receive areas
    port_stop(p);
    pwr(p, P_IS, 0xFFFF_FFFF); // clear pending status
    CLB = pmm::alloc();
    FB = pmm::alloc();
    CTB = pmm::alloc();
    if CLB == 0 || FB == 0 || CTB == 0 {
        serial::log("ahci: out of memory for DMA structures\n");
        return false;
    }
    pwr(p, P_CLB, CLB as u32);
    pwr(p, P_CLBU, (CLB >> 32) as u32);
    pwr(p, P_FB, FB as u32);
    pwr(p, P_FBU, (FB >> 32) as u32);
    // FIS receive on, then command engine on
    pwr(p, P_CMD, prd(p, P_CMD) | (1 << 4));
    let mut n = 1_000_000u32;
    while n > 0 && prd(p, P_CMD) & (1 << 14) == 0 {
        core::hint::spin_loop();
        n -= 1;
    }
    pwr(p, P_CMD, prd(p, P_CMD) | 1);

    READY.store(true, Ordering::Relaxed);
    serial::log("ahci: port ready\n");
    true
}

/// build and issue one command in slot 0; polls until PxCI clears.
/// write = true transfers data to the device.
unsafe fn issue(write: bool, lba: u64, sectors: u32) -> bool {
    let p = (PORT - PORT_OFF) / PORT_STRIDE;

    // command header in slot 0 of the command list
    let cl = CLB as *mut u32;
    let prdtl = (sectors as usize * 512 + MAX_CHUNK - 1) / MAX_CHUNK;
    let w = if write { 1u32 << 6 } else { 0 };
    cl.add(0).write_volatile(
        CFL | w | ((prdtl as u32) << 16), // DW0: FIS length, write, PRDT count
    );
    cl.add(1).write_volatile(0); // DW1: PRDBC (device updates)
    cl.add(2).write_volatile(CTB as u32); // DW2: command table base
    cl.add(3).write_volatile((CTB >> 32) as u32); // DW3: base upper

    // command table: host-to-device FIS (20 bytes used, 64 reserved)
    let ct = CTB as *mut u8;
    let mut fis = [0u8; 64];
    fis[0] = 0x27; // FIS type: host to device
    fis[1] = 0x80; // command FIS, port multiplier 0
    fis[2] = if write { 0x35 } else { 0x25 }; // WRITE/READ DMA EXT
    fis[3] = 0; // features low
    fis[4] = (lba & 0xFF) as u8; // LBA low
    fis[5] = ((lba >> 8) & 0xFF) as u8;
    fis[6] = ((lba >> 16) & 0xFF) as u8;
    fis[7] = 0x40; // device: LBA mode
    fis[8] = ((lba >> 24) & 0xFF) as u8; // LBA high (LBA48)
    fis[9] = ((lba >> 32) & 0xFF) as u8;
    fis[10] = ((lba >> 40) & 0xFF) as u8;
    fis[11] = 0; // features high
    fis[12] = (sectors & 0xFF) as u8; // sector count low
    fis[13] = (sectors >> 8) as u8; // sector count high (LBA48)
    core::ptr::copy_nonoverlapping(fis.as_ptr(), ct, 64);

    // one PRDT entry for this command (chunk <= 32 KiB)
    // entry layout: DBA u64 @0, reserved u32 @8, DBC u32 @12 (byte count - 1)
    let bytes = sectors as usize * 512;
    let prdt = ct.add(128) as *mut u64;
    prdt.write_volatile(DMA.as_ptr() as u64); // data base (u64)
    ((CTB + 128 + 12) as *mut u32).write_volatile(bytes as u32 - 1); // byte count - 1

    // issue slot 0 and poll for completion
    pwr(p, P_CI, 1);
    let mut n = 20_000_000u32; // generous timeout (~ tens of ms)
    while n > 0 {
        if prd(p, P_CI) & 1 == 0 {
            break;
        }
        core::hint::spin_loop();
        n -= 1;
    }
    if n == 0 {
        serial::log("ahci: command timeout\n");
        return false;
    }
    let tfd = prd(p, P_TFD);
    if tfd & (1 << 8) != 0 {
        serial::log("ahci: device error\n");
        return false;
    }
    true
}

/// read `count` 512-byte sectors starting at `lba` into `out`
pub fn read(lba: u64, count: u32, out: &mut [u8]) -> bool {
    if !READY.load(Ordering::Relaxed) || count == 0 {
        return false;
    }
    unsafe {
        let mut done = 0usize;
        while done < count as usize {
            let step = ((count as usize - done).min(MAX_CHUNK / 512)) as u32;
            if !issue(false, lba + done as u64, step) {
                return false;
            }
            let bytes = step as usize * 512;
            core::ptr::copy_nonoverlapping(DMA.as_ptr(), out.as_mut_ptr().add(done * 512), bytes);
            done += step as usize;
        }
    }
    true
}

/// write `count` 512-byte sectors from `data` at `lba`
pub fn write(lba: u64, count: u32, data: &[u8]) -> bool {
    if !READY.load(Ordering::Relaxed) || count == 0 {
        return false;
    }
    if data.len() < count as usize * 512 {
        return false;
    }
    unsafe {
        let mut done = 0usize;
        while done < count as usize {
            let step = ((count as usize - done).min(MAX_CHUNK / 512)) as u32;
            let bytes = step as usize * 512;
            core::ptr::copy_nonoverlapping(
                data.as_ptr().add(done * 512),
                DMA.as_mut_ptr(),
                bytes,
            );
            if !issue(true, lba + done as u64, step) {
                return false;
            }
            done += step as usize;
        }
    }
    true
}

pub fn ready() -> bool {
    READY.load(Ordering::Relaxed)
}
