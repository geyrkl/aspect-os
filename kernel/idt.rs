// ASPECT OS - own IDT: 32 exception stubs + IRQ stubs (global_asm, Intel syntax)
use core::arch::global_asm;
use core::arch::asm;
use crate::pic;
use crate::pit;
use crate::serial;

// stub generator: exceptions 0..=31.
// frame layout after the pushes: [r11 r10 r9 r8 rdi rsi rdx rcx rax][err][RIP][CS][RFLAGS][RSP][SS]
//   err = rsp + 9*8 = rsp + 72
//   RIP = rsp + 10*8 = rsp + 80
// the error code is pushed as a placeholder (0) when the CPU does not push
// one - so the frame always looks the same.
macro_rules! isr_stubs {
    ($($n:literal),* $(,)?) => {
        $(
            global_asm!(concat!(
                "__isr_", stringify!($n), ":\n",
                "push 0\n",
                "push rax\n", "push rcx\n", "push rdx\n", "push rsi\n", "push rdi\n",
                "push r8\n", "push r9\n", "push r10\n", "push r11\n",
                "mov rdi, ", stringify!($n), "\n",
                "mov rsi, [rsp + 72]\n",
                "mov rdx, [rsp + 80]\n",
                "call k_exception_handler\n",
                "pop r11\n", "pop r10\n", "pop r9\n", "pop r8\n", "pop rdi\n",
                "pop rsi\n", "pop rdx\n", "pop rcx\n", "pop rax\n",
                "add rsp, 8\n",
                "iretq\n",
            ));
        )*
    };
}

isr_stubs!(
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
    16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31
);

// IRQ0 timer stub (no error code)
global_asm!(
    "__irq_0:\n",
    "push rax\n", "push rcx\n", "push rdx\n", "push rsi\n", "push rdi\n",
    "push r8\n", "push r9\n", "push r10\n", "push r11\n",
    "mov edi, 0\n",
    "call k_irq_handler\n",
    "pop r11\n", "pop r10\n", "pop r9\n", "pop r8\n", "pop rdi\n",
    "pop rsi\n", "pop rdx\n", "pop rcx\n", "pop rax\n",
    "iretq\n",
);

// IRQ1 keyboard stub
global_asm!(
    "__irq_1:\n",
    "push rax\n", "push rcx\n", "push rdx\n", "push rsi\n", "push rdi\n",
    "push r8\n", "push r9\n", "push r10\n", "push r11\n",
    "mov edi, 1\n",
    "call k_irq_handler\n",
    "pop r11\n", "pop r10\n", "pop r9\n", "pop r8\n", "pop rdi\n",
    "pop rsi\n", "pop rdx\n", "pop rcx\n", "pop rax\n",
    "iretq\n",
);

// IRQ12 mouse stub
global_asm!(
    "__irq_12:\n",
    "push rax\n", "push rcx\n", "push rdx\n", "push rsi\n", "push rdi\n",
    "push r8\n", "push r9\n", "push r10\n", "push r11\n",
    "mov edi, 12\n",
    "call k_irq_handler\n",
    "pop r11\n", "pop r10\n", "pop r9\n", "pop r8\n", "pop rdi\n",
    "pop rsi\n", "pop rdx\n", "pop rcx\n", "pop rax\n",
    "iretq\n",
);

// stub symbols for the IDT table
extern "C" {
    fn __isr_0() -> !;
    fn __isr_1() -> !;
    fn __isr_2() -> !;
    fn __isr_3() -> !;
    fn __isr_4() -> !;
    fn __isr_5() -> !;
    fn __isr_6() -> !;
    fn __isr_7() -> !;
    fn __isr_8() -> !;
    fn __isr_9() -> !;
    fn __isr_10() -> !;
    fn __isr_11() -> !;
    fn __isr_12() -> !;
    fn __isr_13() -> !;
    fn __isr_14() -> !;
    fn __isr_15() -> !;
    fn __isr_16() -> !;
    fn __isr_17() -> !;
    fn __isr_18() -> !;
    fn __isr_19() -> !;
    fn __isr_20() -> !;
    fn __isr_21() -> !;
    fn __isr_22() -> !;
    fn __isr_23() -> !;
    fn __isr_24() -> !;
    fn __isr_25() -> !;
    fn __isr_26() -> !;
    fn __isr_27() -> !;
    fn __isr_28() -> !;
    fn __isr_29() -> !;
    fn __isr_30() -> !;
    fn __isr_31() -> !;
    fn __irq_0() -> !;
    fn __irq_1() -> !;
    fn __irq_12() -> !;
}

#[repr(C)]
#[derive(Clone, Copy)]
struct IdtEntry {
    off_lo: u16,
    sel: u16,
    ist: u8,
    attrs: u8,
    off_mid: u16,
    off_hi: u32,
    zero: u32,
}

const ENTRIES: usize = 48; // 32 exceptions + 16 IRQ vectors

static mut IDT: [IdtEntry; ENTRIES] = unsafe { core::mem::zeroed() };

#[repr(C, packed(1))]
struct IdtPtr {
    limit: u16,
    base: u64,
}

fn set_entry(n: usize, handler: usize) {
    unsafe {
        IDT[n] = IdtEntry {
            off_lo: handler as u16,
            sel: 0x08,
            ist: 0,
            attrs: 0x8E, // present | ring0 | interrupt gate (IF is cleared)
            off_mid: (handler >> 16) as u16,
            off_hi: (handler >> 32) as u32,
            zero: 0,
        };
    }
}

pub unsafe fn init() {
    asm!("cli", options(nomem, nostack));

    let stubs: [usize; ENTRIES] = [
        __isr_0 as usize, __isr_1 as usize, __isr_2 as usize, __isr_3 as usize,
        __isr_4 as usize, __isr_5 as usize, __isr_6 as usize, __isr_7 as usize,
        __isr_8 as usize, __isr_9 as usize, __isr_10 as usize, __isr_11 as usize,
        __isr_12 as usize, __isr_13 as usize, __isr_14 as usize, __isr_15 as usize,
        __isr_16 as usize, __isr_17 as usize, __isr_18 as usize, __isr_19 as usize,
        __isr_20 as usize, __isr_21 as usize, __isr_22 as usize, __isr_23 as usize,
        __isr_24 as usize, __isr_25 as usize, __isr_26 as usize, __isr_27 as usize,
        __isr_28 as usize, __isr_29 as usize, __isr_30 as usize, __isr_31 as usize,
        __irq_0 as usize,
        __irq_1 as usize,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // IRQ2..IRQ11
        __irq_12 as usize,
        0, 0, 0, // IRQ13..IRQ15 later
    ];
    for (n, f) in stubs.iter().enumerate() {
        if *f != 0 {
            set_entry(n, *f);
        }
    }

    let p = IdtPtr {
        limit: (core::mem::size_of::<IdtEntry>() * ENTRIES - 1) as u16,
        base: IDT.as_ptr() as u64,
    };
    asm!("lidt [{p}]", p = in(reg) &p as *const IdtPtr, options(nostack));
    serial::log("idt loaded\n");
}

// exception handler: log and halt for now (M3: panic screen)
#[no_mangle]
extern "C" fn k_exception_handler(n: u64, err: u64, rip: u64) {
    serial::log("!! EXCEPTION ");
    serial::log_num(n as usize);
    serial::log(" err=");
    serial::log_num(err as usize);
    serial::log(" rip=");
    serial::log_hex(rip);
    serial::log("\nkernel halted\n");
    loop {
        unsafe { asm!("cli", "hlt", options(nomem, nostack)) };
    }
}

// IRQ dispatcher: timer, keyboard, mouse
#[no_mangle]
extern "C" fn k_irq_handler(n: u64) {
    if n == 0 {
        pit::tick();
        pic::send_eoi(0);
    } else if n == 1 {
        let ev = unsafe { crate::keyboard::handle() };
        match ev {
            crate::keyboard::KeyEvent::Char(c) => {
                serial::log("key: ");
                let one = [c];
                serial::log(core::str::from_utf8(&one).unwrap_or("?"));
                serial::log("\n");
            }
            crate::keyboard::KeyEvent::Enter => serial::log("key: Enter\n"),
            crate::keyboard::KeyEvent::Backspace => serial::log("key: Backspace\n"),
            crate::keyboard::KeyEvent::Escape => serial::log("key: Escape\n"),
            crate::keyboard::KeyEvent::Ignore => {}
        }
        pic::send_eoi(1);
    } else if n == 12 {
        let raw = crate::io::inb(0x60);
        let (dx, dy, btn, click) = unsafe { crate::mouse::handle(raw) };
        if dx != 0 || dy != 0 || click {
            crate::gui::move_cursor(dx, dy, btn, click);
        }
        pic::send_eoi(12);
    } else {
        // unexpected IRQ: ack it so nothing hangs
        pic::send_eoi(n as u8);
    }
}
