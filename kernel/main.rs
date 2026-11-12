// ASPECT OS - kernel in Rust: module structure
#![no_std]
#![no_main]
use core::arch::asm;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_i: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

pub mod bootinfo;
pub mod font;
pub mod gdt;
pub mod gui;
pub mod idt;
pub mod io;
pub mod keyboard;
pub mod logo;
pub mod mouse;
pub mod pic;
pub mod pit;
pub mod pmm;
pub mod ps2;
pub mod serial;

use bootinfo::BootInfo;

// zero .bss (bootloader hands over raw, uninitialized memory)
unsafe fn zero_bss() {
    extern "C" {
        static __bss_start: u8;
        static __bss_end: u8;
    }
    let start = &__bss_start as *const u8 as usize;
    let end = &__bss_end as *const u8 as usize;
    let mut p = start;
    while p < end {
        (p as *mut u8).write_volatile(0);
        p += 1;
    }
}

#[link_section = ".text.first"]
#[no_mangle]
pub extern "C" fn kmain(bi: *const BootInfo) {
    unsafe { zero_bss() };
    kernel_main(bi);
    loop {
        unsafe { asm!("hlt", options(nostack, nomem)) };
    }
}

fn kernel_main(bi: *const BootInfo) {
    use core::sync::atomic::{AtomicU64, Ordering};
    static LAST_SEC: AtomicU64 = AtomicU64::new(0);

    serial::log("ASPECT OS kernel running (Rust)\n");
    gui::fb_init(&unsafe { *bi });
    gui::draw_gradient();
    serial::log("gradient ok\n");
    gui::draw_menubar();
    serial::log("menubar ok\n");
    logo::draw_wordmark(gui::width() / 2, gui::height() * 40 / 100, 5);
    serial::log("wordmark ok\n");
    gui::draw_dock();
    serial::log("dock ok\n");
    serial::log("desktop drawn\n");

    // --- M2: interrupts ---
    serial::log("M2: setting up GDT\n");
    unsafe { gdt::init() };
    serial::log("gdt loaded\n");
    unsafe { idt::init() };
    serial::log("idt ready\n");
    unsafe { pic::remap() };
    serial::log("pic remapped\n");
    unsafe { pit::init(1000) }; // 1000 Hz
    serial::log("pit at 1000 Hz\n");

    // --- M3: input devices + physical memory allocator ---
    serial::log("M3: ps2 init\n");
    unsafe { ps2::init() };
    pic::unmask_input();
    serial::log("input unmasked\n");

    serial::log("M3: pmm init\n");
    let bi_ref = unsafe { &*bi };
    let mut regs = [pmm::Region { ty: 0, base: 0, pages: 0 }; 96];
    let n = (bi_ref.mem_count as usize).min(96);
    for i in 0..n {
        regs[i] = pmm::Region {
            ty: bi_ref.mem[i].ty,
            base: bi_ref.mem[i].base,
            pages: bi_ref.mem[i].pages,
        };
    }
    unsafe { pmm::init(&regs[..n], 0x100000, bi_ref.kernel_size) };
    serial::log("pmm ready, alloc demo:\n");
    let f1 = pmm::alloc();
    let f2 = pmm::alloc();
    serial::log("frame1 = ");
    serial::log_hex(f1);
    serial::log("  frame2 = ");
    serial::log_hex(f2);
    serial::log("\n");
    pmm::free(f1);
    let f3 = pmm::alloc();
    serial::log("frame3 (after free) = ");
    serial::log_hex(f3);
    serial::log("\n");

    gui::init_cursor();
    serial::log("cursor initialized\n");
    serial::log("interrupts enabled\n");

    // event loop: update the menubar clock every second
    loop {
        unsafe { asm!("sti", options(nomem, nostack)) };
        let sec = pit::ticks() / 1000;
        if LAST_SEC.swap(sec, Ordering::Relaxed) != sec {
            gui::draw_clock(sec);
        }
        unsafe { asm!("hlt", options(nomem, nostack)) };
    }
}
