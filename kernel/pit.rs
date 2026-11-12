// ASPECT OS - PIT (8253/8254, channel 0): system tick
use core::arch::asm;
use core::sync::atomic::{AtomicU64, Ordering};
use crate::io::outb;

static TICKS: AtomicU64 = AtomicU64::new(0);

const PIT_FREQ_HZ: u32 = 1_193_182;

/// set the timer to `hz` ticks per second (channel 0, mode 3, BCD off)
pub unsafe fn init(hz: u64) {
    let div = (PIT_FREQ_HZ / hz as u32) as u16;
    outb(0x43, 0x36); // 00 11 011 0: channel 0, lo+hi, mode 3, binary
    outb(0x40, (div & 0xFF) as u8);
    outb(0x40, (div >> 8) as u8);
}

pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

/// called from the IRQ0 stub
pub fn tick() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}

/// short HLT wait (for later tests)
pub fn wait_ms(ms: u64) {
    let target = ticks() + ms;
    while ticks() < target {
        unsafe { asm!("hlt", options(nomem, nostack)) };
    }
}
