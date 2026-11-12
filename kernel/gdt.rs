// ASPECT OS - own GDT (ring-0 code/data), CS/SS reload after lgdt
use core::arch::asm;

// canonical long-mode descriptors (base 0, limit 0xFFFFF):
//   code: access 0x9A (P|DPL0|S|Exec|Read),  flags 0xAF (G=1, L=1)
//   data: access 0x92 (P|DPL0|S|R/W),        flags 0xCF (G=1, D=1)
static mut GDT: [u64; 3] = [
    0x0000_0000_0000_0000, // null descriptor
    0x00AF_9A00_0000_FFFF, // 0x08: kernel code (64-bit)
    0x00CF_9200_0000_FFFF, // 0x10: kernel data
];

#[repr(C, packed(1))]
struct GdtPtr {
    limit: u16,
    base: u64,
}

pub unsafe fn init() {
    asm!("cli", options(nomem, nostack));
    let p = GdtPtr {
        limit: (core::mem::size_of::<u64>() * GDT.len() - 1) as u16,
        base: GDT.as_ptr() as u64,
    };
    asm!(
        // lgdt + switch CS via far-return into the new code segment
        "lgdt [{p}]",
        "push 0x08",             // new CS
        "lea rax, [rip + 2f]",   // return address behind the retfq
        "push rax",
        "retfq",
        "2:",
        // switch data segments to 0x10
        "mov ax, 0x10",
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",
        "mov ss, ax",
        p = in(reg) &p as *const GdtPtr,
        out("rax") _,
        options(nostack)
    );
}
