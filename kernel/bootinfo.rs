// ASPECT OS - shared structure bootloader -> kernel (Rust)
#![allow(dead_code)]

pub const MAX_MEMREGIONS: usize = 96;

/// compact memory region (own format, copied by the bootloader from the UEFI map)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MemRegion {
    pub ty: u32,    // UEFI type (7 = conventional, 6 = bootloader reclaim usable)
    pub _pad: u32,
    pub base: u64,  // physical base
    pub pages: u64, // 4 KiB pages
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BootInfo {
    pub fb_base: u64,
    pub fb_width: u32,
    pub fb_height: u32,
    pub fb_pitch_px: u32,
    pub fb_bpp: u32,
    pub kernel_size: u64,
    pub mem_count: u32,
    pub _pad: u32,
    pub mem: [MemRegion; MAX_MEMREGIONS],
}
