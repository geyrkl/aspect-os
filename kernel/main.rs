// ASPECT OS - Kernel in Rust: Modulstruktur
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
pub mod gui;
pub mod logo;
pub mod serial;

use bootinfo::BootInfo;

// .bss nullen (Bootloader uebergibt nicht initialisierten Speicher roh)
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
    // bis Milestone 2 (Timer/Eingaben): statisch halten
}
