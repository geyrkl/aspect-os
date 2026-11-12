// ASPECT OS - eigene serielle Debug-Ausgabe (COM1)
#![allow(nonstandard_style)]
use core::arch::asm;

fn outb(port: u16, val: u8) {
    unsafe { asm!("out dx, al", in("al") val, in("dx") port, options(nostack)) }
}
fn inb(port: u16) -> u8 {
    let v: u8;
    unsafe { asm!("in al, dx", out("al") v, in("dx") port, options(nostack)) };
    v
}

pub fn init() {
    outb(0x3F9, 0x00);
    outb(0x3FB, 0x80);
    outb(0x3F8, 0x01);
    outb(0x3F9, 0x00);
    outb(0x3FB, 0x03);
    outb(0x3FA, 0xC7);
    outb(0x3FC, 0x0B);
}

fn putc(c: u8) {
    while inb(0x3FD) & 0x20 == 0 {}
    outb(0x3F8, c);
}

pub fn log(s: &str) {
    for b in s.bytes() {
        putc(b);
    }
}

pub fn log_num(v: usize) {
    let mut buf = [0u8; 20];
    let mut i = 20;
    if v == 0 {
        buf[19] = b'0';
        i = 19;
    } else {
        let mut x = v;
        while x > 0 {
            i -= 1;
            buf[i] = b'0' + (x % 10) as u8;
            x /= 10;
        }
    }
    log(core::str::from_utf8(&buf[i..20]).unwrap_or("?"));
}
