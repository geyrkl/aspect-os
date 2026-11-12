// ASPECT OS - own UEFI bootloader in Rust (no foreign crates, no GNU-EFI)
// Tasks: screen mode (GOP), copy embedded kernel to 0x100000,
//        exit boot services, handover.
// The kernel is embedded directly -> no filesystem needed, one binary = everything.
#![no_std]
#![no_main]
#![allow(nonstandard_style, dead_code)]
use core::arch::asm;
use core::panic::PanicInfo;

// kernel binary is embedded at build time (Makefile builds kernel.bin first)
static KERNEL: &[u8] = include_bytes!("../build/kernel.bin");

#[panic_handler]
fn panic(_i: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

// ---- basic types ----
type UINTN = u64;
type EFI_STATUS = u64;
type EFI_HANDLE = u64;

const EFI_SUCCESS: EFI_STATUS = 0;
const EFI_BUFFER_TOO_SMALL: EFI_STATUS = 0x8000_0000_0000_0005;
const EFI_INVALID_PARAMETER: EFI_STATUS = 0x8000_0000_0000_0002;

const EFI_LOADER_DATA: u32 = 2;
const ALLOCATE_ADDRESS: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy)]
struct Guid {
    a: u32,
    b: u16,
    c: u16,
    d: [u8; 8],
}

const GOP_GUID: Guid = Guid {
    a: 0x9042A9DE,
    b: 0x23DC,
    c: 0x4A38,
    d: [0x96, 0xFB, 0x7A, 0xDE, 0xD0, 0x80, 0x51, 0x6A],
};

// ---- BootInfo (identical to the kernel) ----
#[repr(C)]
#[derive(Clone, Copy)]
struct MemRegion {
    ty: u32,
    _pad: u32,
    base: u64,
    pages: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct BootInfo {
    fb_base: u64,
    fb_width: u32,
    fb_height: u32,
    fb_pitch_px: u32,
    fb_bpp: u32,
    kernel_size: u64,
    mem_count: u32,
    _pad: u32,
    mem: [MemRegion; 96],
}

// ---- UEFI function pointers (extern "C" == MS x64 == EFI ABI) ----
type FnOutputString = extern "C" fn(*mut SimpleTextOut, *const u16) -> EFI_STATUS;
type FnGetMemoryMap = extern "C" fn(
    *mut UINTN,
    *mut MemDesc,
    *mut UINTN,
    *mut UINTN,
    *mut u32,
) -> EFI_STATUS;
type FnAllocatePages = extern "C" fn(u32, u32, UINTN, *mut u64) -> EFI_STATUS;
type FnAllocatePool = extern "C" fn(u32, UINTN, *mut *mut u8) -> EFI_STATUS;
type FnFreePool = extern "C" fn(*mut u8) -> EFI_STATUS;
type FnExitBootServices = extern "C" fn(EFI_HANDLE, UINTN) -> EFI_STATUS;
type FnLocateProtocol = extern "C" fn(*const Guid, *const u8, *mut *mut u8) -> EFI_STATUS;
type FnSetWatchdog = extern "C" fn(UINTN, u64, UINTN, *const u16) -> EFI_STATUS;

#[repr(C)]
struct SimpleTextOut {
    _reset: usize,
    output_string: FnOutputString,
    _test_string: usize,
    _query_mode: usize,
    _set_mode: usize,
    _set_attr: usize,
    _clear_screen: usize,
    _enable_cursor: usize,
}

#[repr(C)]
struct MemDesc {
    mtype: u32,
    _pad: u32,
    phys_start: u64,
    virt_start: u64,
    num_pages: u64,
    attr: u64,
}

#[repr(C)]
struct BootServices {
    _hdr: [u64; 3],
    _raise_tpl: usize,                   // 1
    _restore_tpl: usize,                 // 2
    allocate_pages: FnAllocatePages,     // 3
    _free_pages: usize,                  // 4
    get_memory_map: FnGetMemoryMap,      // 5
    allocate_pool: FnAllocatePool,       // 6
    free_pool: FnFreePool,               // 7
    _create_event: usize,                // 8
    _set_timer: usize,                   // 9
    _wait_for_event: usize,              // 10
    _signal_event: usize,                // 11
    _close_event: usize,                 // 12
    _check_event: usize,                 // 13
    _install_protocol: usize,            // 14
    _reinstall_protocol: usize,          // 15
    _uninstall_protocol: usize,          // 16
    _handle_protocol: usize,             // 17
    _reserved: usize,                    // 18
    _register_protocol_notify: usize,    // 19
    _locate_handle: usize,               // 20
    _locate_device_path: usize,          // 21
    _install_configuration_table: usize, // 22
    _load_image: usize,                  // 23
    _start_image: usize,                 // 24
    _exit: usize,                        // 25
    _unload_image: usize,                // 26
    exit_boot_services: FnExitBootServices, // 27
    _get_next_monotonic_count: usize,    // 28
    _stall: usize,                       // 29
    set_watchdog_timer: FnSetWatchdog,   // 30
    _connect_controller: usize,          // 31
    _disconnect_controller: usize,       // 32
    _open_protocol: usize,               // 33
    _close_protocol: usize,              // 34
    _open_protocol_information: usize,   // 35
    _protocols_per_handle: usize,        // 36
    _locate_handle_buffer: usize,        // 37
    locate_protocol: FnLocateProtocol,   // 38
    _install_multi: usize,               // 39
    _uninstall_multi: usize,             // 40
    _calculate_crc32: usize,             // 41
    _copy_mem: usize,                    // 42
    _set_mem: usize,                     // 43
    _create_event_ex: usize,             // 44
}

#[repr(C)]
struct SystemTable {
    _hdr: [u64; 3],
    _fw_vendor: u64,
    _fw_revision: u32,
    _pad: u32,
    _console_in_handle: u64,
    _con_in: u64,
    _console_out_handle: u64,
    con_out: *mut SimpleTextOut,
    _std_err_handle: u64,
    _std_err: u64,
    _runtime_services: u64,
    boot_services: *mut BootServices,
    _n_table_entries: UINTN,
    _config_table: u64,
}

// Graphics Output Protocol
type FnGopQueryMode =
    extern "C" fn(*mut Gop, u32, *mut UINTN, *mut *mut GopModeInfo) -> EFI_STATUS;
type FnGopSetMode = extern "C" fn(*mut Gop, u32) -> EFI_STATUS;

#[repr(C)]
struct Gop {
    query_mode: FnGopQueryMode,
    set_mode: FnGopSetMode,
    _blt: usize,
    mode: *mut GopMode,
}

#[repr(C)]
struct GopModeInfo {
    _version: u32,
    h_res: u32,
    v_res: u32,
    pixel_format: u32,
    _masks: [u32; 4],
    scanline_px: u32,
}

#[repr(C)]
struct GopMode {
    max_mode: u32,
    _mode: u32,
    info: *mut GopModeInfo,
    _size_of_info: UINTN,
    fb_base: u64,
    _fb_size: UINTN,
}

// ---- serial (COM1, own routines, Intel-syntax asm) ----
fn outb(port: u16, val: u8) {
    unsafe { asm!("out dx, al", in("al") val, in("dx") port, options(nostack)) }
}
fn inb(port: u16) -> u8 {
    let v: u8;
    unsafe { asm!("in al, dx", out("al") v, in("dx") port, options(nostack)) };
    v
}

fn serial_init() {
    outb(0x3F9, 0x00);
    outb(0x3FB, 0x80);
    outb(0x3F8, 0x01);
    outb(0x3F9, 0x00);
    outb(0x3FB, 0x03);
    outb(0x3FA, 0xC7);
    outb(0x3FC, 0x0B);
}
fn serial_putc(c: u8) {
    while inb(0x3FD) & 0x20 == 0 {}
    outb(0x3F8, c);
}
fn serial_log(s: &str) {
    for b in s.bytes() {
        serial_putc(b);
    }
}

// ---- console + errors ----
struct Ctx {
    st: *mut SystemTable,
    gop: *mut Gop,
}

unsafe fn con_print(ctx: &Ctx, s: &str) {
    if (*ctx.st).con_out.is_null() {
        return;
    }
    for b in s.bytes() {
        ((*(*ctx.st).con_out).output_string)((*ctx.st).con_out, [b as u16, 0].as_ptr());
    }
}

unsafe fn fatal(ctx: &Ctx, msg: &str) -> ! {
    serial_log("BOOT FATAL: ");
    serial_log(msg);
    serial_log("\n");
    con_print(ctx, "ASPECT OS boot failure: ");
    con_print(ctx, msg);
    con_print(ctx, "\r\n");
    if !(*ctx.gop).mode.is_null() {
        let fb = (*(*ctx.gop).mode).fb_base as *mut u32;
        let px = (*(*(*ctx.gop).mode).info).scanline_px as usize;
        let h = (*(*(*ctx.gop).mode).info).v_res as usize;
        for y in 0..h {
            for x in 0..24 {
                fb.add(y * px + x).write_volatile(0xFFD03A2F);
            }
        }
    }
    loop {
        asm!("hlt", options(nostack, nomem));
    }
}

// ---- handover: switch stack, rdi = BootInfo (fixed in rdi), call kernel ----
unsafe fn handoff(entry: u64, bi: u64, stack: u64) -> ! {
    asm!(
        "mov rsp, {stk}",
        "xor ebp, ebp",
        "call {ent}",
        ent = in(reg) entry,
        in("rdi") bi,
        stk = in(reg) stack,
        options(noreturn)
    );
}

#[no_mangle]
extern "C" fn efi_main(image: EFI_HANDLE, st: *mut SystemTable) -> EFI_STATUS {
    serial_init();
    serial_log("\n=== ASPECT OS bootloader (Rust) ===\n");
    let mut ctx = Ctx { st, gop: core::ptr::null_mut() };
    unsafe {
        let bs = (*st).boot_services;
        ((*bs).set_watchdog_timer)(0, 0, 0, core::ptr::null());
        con_print(&ctx, "ASPECT OS bootloader\r\n");

        // --- GOP: find the highest resolution ---
        let mut gop_ptr: *mut u8 = core::ptr::null_mut();
        if ((*bs).locate_protocol)(&GOP_GUID, core::ptr::null(), &mut gop_ptr) != EFI_SUCCESS {
            fatal(&ctx, "no GOP");
        }
        ctx.gop = gop_ptr as *mut Gop;
        let gop = ctx.gop;

        let mut best: u32 = 0;
        let mut best_area: u64 = 0;
        let mut found = false;
        for i in 0..(*gop).mode.as_ref().map_or(0, |m| m.max_mode) {
            let mut sz: UINTN = 0;
            let mut info: *mut GopModeInfo = core::ptr::null_mut();
            if ((*gop).query_mode)(gop, i, &mut sz, &mut info) != EFI_SUCCESS {
                continue;
            }
            if (*info).pixel_format != 1 {
                // 1 = BlueGreenRedReserved (0xAARRGGBB in memory)
                continue;
            }
            let area = (*info).h_res as u64 * (*info).v_res as u64;
            if area > best_area {
                best = i;
                best_area = area;
                found = true;
            }
        }
        if !found {
            fatal(&ctx, "no suitable video mode");
        }
        if ((*gop).set_mode)(gop, best) != EFI_SUCCESS {
            fatal(&ctx, "SetMode failed");
        }
        // screen black
        let fb = (*(*gop).mode).fb_base as *mut u32;
        let px = (*(*(*gop).mode).info).scanline_px as usize;
        let w = (*(*(*gop).mode).info).h_res as usize;
        let h = (*(*(*gop).mode).info).v_res as usize;
        for y in 0..h {
            for x in 0..w {
                fb.add(y * px + x).write_volatile(0xFF101014);
            }
        }
        con_print(&ctx, "video mode set\r\n");
        serial_log("video mode set\n");

        // --- kernel: embedded directly, copy to 0x100000 ---
        let ksize = KERNEL.len() as u64;
        let mut kbase: u64 = 0x100000;
        let kpages = ((ksize + 0xFFFFF) >> 20).max(4);
        if ((*bs).allocate_pages)(ALLOCATE_ADDRESS, EFI_LOADER_DATA, kpages, &mut kbase)
            != EFI_SUCCESS
        {
            fatal(&ctx, "AllocatePages 0x100000 failed");
        }
        let dst = kbase as *mut u8;
        for (i, b) in KERNEL.iter().enumerate() {
            dst.add(i).write_volatile(*b);
        }
        serial_log("kernel at 0x100000\n");

        // --- BootInfo ---
        let mut bi_ptr: *mut u8 = core::ptr::null_mut();
        if ((*bs).allocate_pool)(
            EFI_LOADER_DATA,
            core::mem::size_of::<BootInfo>() as UINTN,
            &mut bi_ptr,
        ) != EFI_SUCCESS
        {
            fatal(&ctx, "AllocatePool BootInfo failed");
        }
        let bi = bi_ptr as *mut BootInfo;
        // pool memory is not zeroed -> clear the whole BootInfo
        let bi_bytes = core::mem::size_of::<BootInfo>();
        for i in 0..bi_bytes {
            (bi_ptr.add(i)).write_volatile(0);
        }
        (*bi).fb_base = (*(*gop).mode).fb_base;
        (*bi).fb_width = (*(*(*gop).mode).info).h_res;
        (*bi).fb_height = (*(*(*gop).mode).info).v_res;
        (*bi).fb_pitch_px = (*(*(*gop).mode).info).scanline_px;
        (*bi).fb_bpp = 32;
        (*bi).kernel_size = ksize;

        // --- own stack ---
        let mut stack_mem: *mut u8 = core::ptr::null_mut();
        if ((*bs).allocate_pool)(EFI_LOADER_DATA, 65536, &mut stack_mem) != EFI_SUCCESS {
            fatal(&ctx, "AllocatePool stack failed");
        }
        let stack = (stack_mem as u64) + 65536;

        // --- memory map + ExitBootServices (with retry) ---
        con_print(&ctx, "handing off...\r\n");
        serial_log("exiting boot services\n");
        let mut msize: UINTN = 0;
        let mut map: *mut u8 = core::ptr::null_mut();
        for _ in 0..8 {
            let mut mkey: UINTN = 0;
            let mut dsize: UINTN = 0;
            let mut dver: u32 = 0;
            let st1 = ((*bs).get_memory_map)(
                &mut msize,
                map as *mut MemDesc,
                &mut mkey,
                &mut dsize,
                &mut dver,
            );
            if st1 == EFI_BUFFER_TOO_SMALL {
                if !map.is_null() {
                    ((*bs).free_pool)(map);
                }
                msize += dsize * 4;
                if ((*bs).allocate_pool)(EFI_LOADER_DATA, msize, &mut map) != EFI_SUCCESS {
                    fatal(&ctx, "AllocatePool map failed");
                }
                continue;
            }
            if st1 != EFI_SUCCESS {
                fatal(&ctx, "GetMemoryMap failed");
            }
            // copy the memory map into BootInfo (before ExitBootServices)
            let count = (msize / dsize) as u32;
            let src = map as *const MemDesc;
            let mut i: u32 = 0;
            while i < count && (i as usize) < 96 {
                let d = src.add(i as usize);
                (*bi).mem[i as usize].ty = (*d).mtype;
                (*bi).mem[i as usize].base = (*d).phys_start;
                (*bi).mem[i as usize].pages = (*d).num_pages;
                i += 1;
            }
            (*bi).mem_count = i;
            serial_log("memmap ok\n");
            let st2 = ((*bs).exit_boot_services)(image, mkey);
            if st2 == EFI_INVALID_PARAMETER {
                continue; // map changed -> fetch again
            }
            if st2 != EFI_SUCCESS {
                fatal(&ctx, "ExitBootServices failed");
            }
            handoff(kbase, bi as u64, stack);
        }
        fatal(&ctx, "ExitBootServices retry exhausted");
    }
}
