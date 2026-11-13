// ASPECT OS - own serial debug output (COM1)
#![allow(nonstandard_style)]
use crate::io::{inb, outb};

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

/// log raw bytes (debug helpers for non-UTF8 buffers)
pub fn log_bytes(b: &[u8]) {
    for &c in b {
        putc(c);
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

pub fn log_hex(v: u64) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut buf = [0u8; 18];
    buf[0] = b'0';
    buf[1] = b'x';
    let mut x = v;
    let mut i = 17;
    if x == 0 {
        buf[2] = b'0';
        i = 2;
    }
    while x > 0 {
        buf[i] = HEX[(x & 0xF) as usize];
        x >>= 4;
        i -= 1;
    }
    log("0x");
    log(core::str::from_utf8(&buf[i + 1..18]).unwrap_or("?"));
}
