// ASPECT OS - own physical memory allocator (bitmap, 4 KiB frames)
use core::sync::atomic::{AtomicU64, Ordering};

const FRAME: u64 = 4096;
const MAX_GB: u64 = 2; // covers 2 GiB (QEMU test RAM); dynamic sizing for real HW later
const BITS: usize = (MAX_GB * 1024 * 1024 * 1024 / FRAME) as usize;
const WORDS: usize = BITS / 64;

static mut BITMAP: [u64; WORDS] = [!0u64; WORDS]; // everything "used" until init()
static CURSOR: AtomicU64 = AtomicU64::new(0);
static USED: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
pub struct Region {
    pub ty: u32,
    pub base: u64,
    pub pages: u64,
}

#[inline]
fn set_bit(n: usize, val: bool) {
    unsafe {
        let w = n / 64;
        let b = n % 64;
        if val {
            BITMAP[w] |= 1 << b;
        } else {
            BITMAP[w] &= !(1 << b);
        }
    }
}

#[inline]
fn get_bit(n: usize) -> bool {
    unsafe { BITMAP[n / 64] & (1 << (n % 64)) != 0 }
}

/// Free frames from the boot memory map, then re-reserve the kernel.
pub unsafe fn init(regions: &[Region], kernel_base: u64, kernel_size: u64) {
    // mark everything as used
    for w in 0..WORDS {
        BITMAP[w] = !0u64;
    }

    // free usable regions (7 = conventional, 6 = bootloader reclaim)
    for r in regions {
        if r.ty != 7 && r.ty != 6 {
            continue;
        }
        let start = (r.base + FRAME - 1) & !(FRAME - 1);
        let end = r.base.saturating_add(r.pages * FRAME) & !(FRAME - 1);
        let mut f = start / FRAME;
        let last = end / FRAME;
        while f < last && f < BITS as u64 {
            set_bit(f as usize, false);
            f += 1;
        }
    }

    // reserve the kernel itself plus everything below 1 MiB (frame indices!)
    mark_used(0, 0x100000 / FRAME);
    let kstart = kernel_base & !(FRAME - 1);
    let klen = kernel_size + (kernel_base - kstart);
    let kpages = (klen + FRAME - 1) / FRAME;
    mark_used(kstart / FRAME, kstart / FRAME + kpages);

    USED.store(0, Ordering::Relaxed);
}

fn mark_used(first: u64, after: u64) {
    let mut f = first;
    while f < after.min(BITS as u64) {
        set_bit(f as usize, true);
        f += 1;
    }
}

/// allocate one frame (physical address), 0 = out of memory
pub fn alloc() -> u64 {
    let start = CURSOR.load(Ordering::Relaxed);
    let mut n = start;
    loop {
        if get_bit(n as usize) == false {
            set_bit(n as usize, true);
            CURSOR.store(n, Ordering::Relaxed);
            USED.fetch_add(1, Ordering::Relaxed);
            return n * FRAME;
        }
        n = (n + 1) % BITS as u64;
        if n == start {
            return 0;
        }
    }
}

pub fn free(base: u64) {
    let n = base / FRAME;
    if n < BITS as u64 {
        set_bit(n as usize, false);
        USED.fetch_sub(1, Ordering::Relaxed);
    }
}

pub fn used_frames() -> u64 {
    USED.load(Ordering::Relaxed)
}
