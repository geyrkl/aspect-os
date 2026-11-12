// ASPECT OS - PS/2 controller (8042): init for keyboard + mouse
use crate::io::{inb, outb};
use core::sync::atomic::{AtomicBool, Ordering};

pub static MOUSE_PRESENT: AtomicBool = AtomicBool::new(false);

const DATA: u16 = 0x60;
const CMD: u16 = 0x64;

fn status() -> u8 {
    inb(CMD)
}

fn out_buf_full() -> bool {
    status() & 0x02 != 0
}

fn in_buf_full() -> bool {
    status() & 0x01 != 0
}

fn wait_can_write() {
    let mut guard = 100_000u32;
    while out_buf_full() && guard > 0 {
        guard -= 1;
    }
}

fn flush_output() {
    let mut guard = 1000u32;
    while in_buf_full() && guard > 0 {
        inb(DATA);
        guard -= 1;
    }
}

fn command(c: u8) {
    wait_can_write();
    outb(CMD, c);
}

fn send_port1(d: u8) {
    wait_can_write();
    outb(DATA, d);
}

fn send_port2(d: u8) {
    command(0xD4); // next byte goes to port 2 (mouse)
    wait_can_write();
    outb(DATA, d);
}

fn wait_ack() -> bool {
    let mut guard = 200_000u32;
    while guard > 0 {
        if in_buf_full() {
            let b = inb(DATA);
            if b == 0xFA {
                return true;
            }
            if b == 0xFE {
                return false; // resend -> give up
            }
        }
        guard -= 1;
    }
    false
}

/// initialize controller and both devices; device IRQs stay masked for now.
pub unsafe fn init() {
    command(0xAD); // port 1 off
    command(0xA7); // port 2 off
    flush_output();

    // read config: enable IRQs for both ports, release clocks,
    // translation (bit 6) off -> we force scancode set 2 ourselves
    command(0x20);
    let mut cfg = 0u8;
    let mut guard = 100_000u32;
    while guard > 0 && !in_buf_full() {
        guard -= 1;
    }
    if in_buf_full() {
        cfg = inb(DATA);
    }
    cfg |= 0x03;        // enable IRQ1 + IRQ12
    cfg &= !0x40;       // translation off
    cfg &= !0x10;       // port 1 clock on
    cfg &= !0x20;       // port 2 clock on
    command(0x60);
    send_port1(cfg);

    command(0xAE); // port 1 on
    command(0xA8); // port 2 on

    // --- keyboard: force scancode set 2, enable scanning ---
    send_port1(0xF0);
    send_port1(0x02);
    wait_ack();
    send_port1(0xF4); // enable scanning
    let kb = wait_ack();
    crate::serial::log("ps2 keyboard ");
    crate::serial::log(if kb { "ok\n" } else { "missing\n" });

    // --- mouse: defaults + reporting on ---
    send_port2(0xF6); // default settings
    let m1 = wait_ack();
    send_port2(0xF4); // enable data reporting
    let m2 = wait_ack();
    flush_output(); // drain stray bytes (late acks etc.) so the mouse stream starts clean
    let ok = m1 && m2;
    MOUSE_PRESENT.store(ok, Ordering::Relaxed);
    crate::serial::log("ps2 mouse ");
    crate::serial::log(if ok { "ok\n" } else { "missing\n" });
}
