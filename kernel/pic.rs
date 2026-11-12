// ASPECT OS - 8259A PIC: remap to vectors 32..47, only IRQ0 unmasked initially
use crate::io::outb;

const PIC1_CMD: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_CMD: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;
const EOI: u8 = 0x20;

pub unsafe fn remap() {
    outb(PIC1_CMD, 0x11); // ICW1: init, ICW4 follows
    io_wait();
    outb(PIC2_CMD, 0x11);
    io_wait();
    outb(PIC1_DATA, 0x20); // ICW2: IRQ0..7 -> vectors 32..39
    io_wait();
    outb(PIC2_DATA, 0x28); // ICW2: IRQ8..15 -> vectors 40..47
    io_wait();
    outb(PIC1_DATA, 0x04); // ICW3: slave on IRQ2
    io_wait();
    outb(PIC2_DATA, 0x02);
    io_wait();
    outb(PIC1_DATA, 0x01); // ICW4: 8086 mode
    io_wait();
    outb(PIC2_DATA, 0x01);
    io_wait();
    outb(PIC1_DATA, 0xFE); // mask: only IRQ0 (timer) active
    outb(PIC2_DATA, 0xFF); // slave fully masked
}

fn io_wait() {
    outb(0x80, 0);
}

pub fn send_eoi(irq: u8) {
    if irq >= 8 {
        outb(PIC2_CMD, EOI);
    }
    outb(PIC1_CMD, EOI);
}

/// unmask IRQ0 (timer), IRQ1 (keyboard) + IRQ12 (mouse)
pub fn unmask_input() {
    unsafe {
        outb(PIC1_DATA, 0xF8); // IRQ0, IRQ1, IRQ2 (cascade)
        outb(PIC2_DATA, 0xEF); // IRQ12
    }
}
