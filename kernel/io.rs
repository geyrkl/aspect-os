// ASPECT OS - port I/O (own routines, Intel-syntax asm)
use core::arch::asm;

pub fn outb(port: u16, val: u8) {
    unsafe { asm!("out dx, al", in("al") val, in("dx") port, options(nostack)) }
}

pub fn inb(port: u16) -> u8 {
    let v: u8;
    unsafe { asm!("in al, dx", out("al") v, in("dx") port, options(nostack)) };
    v
}
