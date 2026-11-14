// ASPECT OS - kernel in Rust: module structure
#![no_std]
#![no_main]
#![allow(unknown_lints, static_mut_refs)]
use core::arch::asm;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(i: &PanicInfo) -> ! {
    serial::log("KERNEL PANIC: ");
    if let Some(m) = i.message().as_str() {
        serial::log(m);
    } else if let Some(loc) = i.location() {
        serial::log("at ");
        serial::log(loc.file());
        serial::log(":");
        serial::log_num(loc.line() as usize);
    }
    serial::log("\nkernel halted\n");
    loop {
        unsafe { asm!("cli", "hlt", options(nomem, nostack)) };
    }
}

pub mod apps;
pub mod ahci;
pub mod bootinfo;
pub mod e1000;
pub mod font;
pub mod fs;
pub mod gdt;
pub mod gui;
pub mod idt;
pub mod io;
pub mod keyboard;
pub mod logo;
pub mod mouse;
pub mod net;
pub mod pci;
pub mod pic;
pub mod pit;
pub mod pmm;
pub mod ps2;
pub mod serial;
pub mod wm;

use apps::AppId;
use bootinfo::BootInfo;

// zero .bss (bootloader hands over raw, uninitialized memory);
// u64 bulk writes, byte-wise head/tail for alignment
unsafe fn zero_bss() {
    extern "C" {
        static __bss_start: u8;
        static __bss_end: u8;
    }
    let start = &__bss_start as *const u8 as usize;
    let end = &__bss_end as *const u8 as usize;
    let mut p = start;
    while p < end && (p & 7) != 0 {
        (p as *mut u8).write_volatile(0);
        p += 1;
    }
    let mut q = p as *mut u64;
    while (q as usize) + 8 <= end {
        q.write_volatile(0);
        q = q.add(1);
    }
    let mut r = q as *mut u8;
    while (r as usize) < end {
        r.write_volatile(0);
        r = r.add(1);
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

    // --- M4: compositor boot-up ---
    gui::fb_init(&unsafe { *bi });
    wm::init();
    apps::init();
    gui::bg_init();
    serial::log("desktop background ok\n");

    // initial windows: file manager, terminal, calculator
    wm::open(AppId::Files);
    wm::open(AppId::Terminal);
    wm::open(AppId::Calc);
    wm::redraw();
    serial::log("first frame drawn\n");

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
    unsafe { pmm::init(&regs[..n], 0x2000000, bi_ref.kernel_size) };
    serial::log("pmm ready\n");

    // --- M5: real file system (own PCI scan + AHCI driver + ASPFS) ---
    serial::log("M5: pci scan\n");
    unsafe { pci::init() };
    serial::log("M5: ahci init\n");
    unsafe { ahci::init(pci::bar()) };
    serial::log("M5: fs mount\n");
    unsafe { fs::mount() };
    apps::fs_populate();
    serial::log("fs lists ready\n");

    // --- M6: network stack (own PCI scan + e1000 driver + ARP/IP/ICMP) ---
    serial::log("M6: net init\n");
    if net::init() {
        serial::log("net ready\n");
    } else {
        serial::log("net: no network hardware\n");
    }

    gui::init_cursor();
    serial::log("M4: compositor running, interrupts enabled\n");

    // event loop: drain input queues, drive animations, redraw on change,
    // wake once per second so the menubar clock updates
    loop {
        unsafe { asm!("sti", options(nomem, nostack)) };
        wm::on_mouse();
        wm::on_key();
        net::poll(); // M6: poll the NIC (no interrupts)
        if wm::animating() {
            wm::mark_dirty(); // keep the frames coming while anything moves
        }
        if wm::take_dirty() {
            wm::redraw();
        }
        let sec = pit::ticks() / 1000;
        if LAST_SEC.swap(sec, Ordering::Relaxed) != sec {
            // idle path: only repaint the menubar clock region instead of a
            // full frame (the background bar is baked without the clock)
            if wm::animating() || wm::dirty() {
                wm::mark_dirty();
            } else {
                gui::draw_clock(sec);
            }
        }
        unsafe { asm!("hlt", options(nomem, nostack)) };
    }
}
