// ASPECT OS - own Intel e1000 (8254x) ethernet driver. Polling, no
// interrupts: reset the controller, install TX/RX descriptor rings in
// static memory (identity-mapped physical addresses), then move packets
// with the ring head/tail registers. The MAC address is read from the
// receive-address registers the device holds.
#![allow(unknown_lints, static_mut_refs, dead_code)]

use crate::serial;
use core::sync::atomic::{AtomicBool, Ordering};

// register offsets (BAR0, 32-bit MMIO)
const CTRL: usize = 0x0000;
const STATUS: usize = 0x0008;
const IMC: usize = 0x00D8; // interrupt mask clear (we poll)
const RCTL: usize = 0x0100;
const TCTL: usize = 0x0400;
const RDBAL: usize = 0x2800;
const RDBAH: usize = 0x2804;
const RDLEN: usize = 0x2808;
const RDH: usize = 0x2810;
const RDT: usize = 0x2818;
const TDBAL: usize = 0x3800;
const TDBAH: usize = 0x3804;
const TDLEN: usize = 0x3808;
const TDH: usize = 0x3810;
const TDT: usize = 0x3818;
const RAL: usize = 0x5400;
const RAH: usize = 0x5404;

// CTRL bits
const CTRL_RST: u32 = 1 << 26;
const CTRL_SLU: u32 = 1 << 6;
// RCTL bits
const RCTL_EN: u32 = 1 << 1;
const RCTL_UPE: u32 = 1 << 3;
const RCTL_MPE: u32 = 1 << 4;
const RCTL_BAM: u32 = 1 << 15;
// TCTL bits
const TCTL_EN: u32 = 1 << 1;
const TCTL_PSP: u32 = 1 << 3;
const TCTL_CT: u32 = 0xF << 4;
const TCTL_COLD: u32 = 0x3F << 12;
// descriptor command bits (TX, in the upper byte of lower.data)
const CMD_EOP: u32 = 1 << 24;
const CMD_IFCS: u32 = 1 << 25;
const CMD_RS: u32 = 1 << 27;
// descriptor status bits
const ST_DD: u8 = 1; // descriptor done
const ST_EOP: u8 = 4; // end of packet

const TX_N: usize = 8; // TX ring slots
const RX_N: usize = 32; // RX ring slots
const BUF: usize = 2048; // buffer size per descriptor

#[derive(Clone, Copy)]
#[repr(C)]
struct TxD {
    addr: u64,
    lower: u32, // length (16) | cso (8) | cmd (8)
    upper: u32, // status (8) | cso (8) | special (16)
}

#[derive(Clone, Copy)]
#[repr(C)]
struct RxD {
    addr: u64,
    length: u16,
    csum: u16,
    status: u8,
    errors: u8,
    special: u16,
}

// descriptor rings must be 128-byte aligned
#[repr(C, align(128))]
struct Rings {
    tx: [TxD; TX_N],
    rx: [RxD; RX_N],
}

static mut RINGS: Rings = Rings {
    tx: [TxD { addr: 0, lower: 0, upper: 0 }; TX_N],
    rx: [RxD { addr: 0, length: 0, csum: 0, status: 0, errors: 0, special: 0 }; RX_N],
};
static mut TXBUF: [[u8; BUF]; TX_N] = [[0; BUF]; TX_N];
static mut RXBUF: [[u8; BUF]; RX_N] = [[0; BUF]; RX_N];

static mut BASE: u64 = 0;
static mut MAC: [u8; 6] = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
static mut TXI: usize = 0; // next TX slot to fill
static mut RXI: usize = 0; // next RX descriptor to read
static READY: AtomicBool = AtomicBool::new(false);

#[inline]
unsafe fn rd(off: usize) -> u32 {
    ((BASE + off as u64) as *mut u32).read_volatile()
}

#[inline]
unsafe fn wr(off: usize, val: u32) {
    ((BASE + off as u64) as *mut u32).write_volatile(val);
}

/// MAC address of the controller
pub fn mac() -> [u8; 6] {
    unsafe { MAC }
}

pub fn ready() -> bool {
    READY.load(Ordering::Relaxed)
}

/// initialize the controller at MMIO base `bar`
pub unsafe fn init(bar: u64) -> bool {
    READY.store(false, Ordering::Relaxed);
    if bar == 0 {
        serial::log("e1000: no controller found\n");
        return false;
    }
    BASE = bar;

    // disable our interrupt use, reset the device, then set the link up
    wr(IMC, 0xFFFF_FFFF);
    wr(CTRL, rd(CTRL) | CTRL_RST);
    let mut n = 2_000_000u32;
    while n > 0 && rd(CTRL) & CTRL_RST != 0 {
        core::hint::spin_loop();
        n -= 1;
    }
    wr(CTRL, rd(CTRL) | CTRL_SLU);

    // MAC address: RAL = bytes 0..3, RAH = bytes 4..5 (little endian)
    let ral = rd(RAL);
    let rah = rd(RAH);
    MAC[0] = ral as u8;
    MAC[1] = (ral >> 8) as u8;
    MAC[2] = (ral >> 16) as u8;
    MAC[3] = (ral >> 24) as u8;
    MAC[4] = rah as u8;
    MAC[5] = (rah >> 8) as u8;
    let mut mac_ok = true;
    for i in 0..6 {
        if MAC[i] == 0 {
            mac_ok = false;
        }
    }
    if !mac_ok {
        // fall back to the well-known QEMU address
        MAC = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
    }

    // --- TX ring ---
    for i in 0..TX_N {
        RINGS.tx[i] = TxD { addr: 0, lower: 0, upper: 0 };
    }
    wr(TDBAL, (&RINGS.tx as *const TxD as u64) as u32);
    wr(TDBAH, (((&RINGS.tx as *const TxD as u64) >> 32) as u32));
    wr(TDLEN, (TX_N * 16) as u32);
    wr(TDH, 0);
    wr(TDT, 0);
    TXI = 0;

    // --- RX ring ---
    for i in 0..RX_N {
        let buf = RXBUF[i].as_ptr() as u64;
        RINGS.rx[i] = RxD {
            addr: buf,
            length: 0,
            csum: 0,
            status: 0,
            errors: 0,
            special: 0,
        };
    }
    wr(RDBAL, (&RINGS.rx as *const RxD as u64) as u32);
    wr(RDBAH, (((&RINGS.rx as *const RxD as u64) >> 32) as u32));
    wr(RDLEN, (RX_N * 16) as u32);
    wr(RDH, 0);
    RXI = 0;
    wr(RDT, (RX_N - 1) as u32); // hardware may use every descriptor

    // enable receive and transmit
    wr(RCTL, RCTL_EN | RCTL_UPE | RCTL_MPE | RCTL_BAM);
    wr(TCTL, TCTL_EN | TCTL_PSP | TCTL_CT | TCTL_COLD);

    READY.store(true, Ordering::Relaxed);
    serial::log("e1000: ready, MAC up\n");
    true
}

/// send one frame (blocking until the descriptor completes)
pub fn send(frame: &[u8]) -> bool {
    if !READY.load(Ordering::Relaxed) || frame.is_empty() || frame.len() > BUF {
        return false;
    }
    unsafe {
        let i = TXI;
        TXBUF[i][..frame.len()].copy_from_slice(frame);
        let slot = TXBUF[i].as_ptr() as u64;
        RINGS.tx[i].addr = slot;
        RINGS.tx[i].lower = frame.len() as u32 | CMD_EOP | CMD_IFCS | CMD_RS;
        RINGS.tx[i].upper = 0;
        TXI = (i + 1) % TX_N;
        wr(TDT, TXI as u32); // ring the doorbell (QEMU transmits here)
        // wait for the descriptor-done status
        let mut n = 4_000_000u32;
        while n > 0 && (RINGS.tx[i].upper & 0xFF) & (ST_DD as u32) == 0 {
            core::hint::spin_loop();
            n -= 1;
        }
        n > 0
    }
}

/// drain completed RX descriptors, calling `f` for each frame
pub fn poll(mut f: impl FnMut(&[u8])) {
    if !READY.load(Ordering::Relaxed) {
        return;
    }
    unsafe {
        loop {
            let d = &mut RINGS.rx[RXI];
            if d.status & ST_DD == 0 {
                break;
            }
            let len = d.length as usize;
            let eop = d.status & ST_EOP != 0;
            d.status = 0; // give the descriptor back
            if len > 0 && eop {
                f(&RXBUF[RXI][..len]);
            }
            wr(RDT, RXI as u32); // hand the slot back to the hardware
            RXI = (RXI + 1) % RX_N;
        }
    }
}
