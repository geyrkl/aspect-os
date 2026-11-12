// ASPECT OS - geteilte Struktur Bootloader -> Kernel (Rust)
#![allow(dead_code)]

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BootInfo {
    pub fb_base: u64,
    pub fb_width: u32,
    pub fb_height: u32,
    pub fb_pitch_px: u32,
    pub fb_bpp: u32,
    pub kernel_size: u64,
}
