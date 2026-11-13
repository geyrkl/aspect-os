// ASPECT OS - ASPFS, the own file system (M5). A superblock, a block
// allocation bitmap, chained directory blocks with 128-byte entries, and
// files stored as contiguous extents. All metadata is cached in static
// memory and written back through the AHCI driver; no heap, no external
// file system code.
//
// block layout: 4 KiB = 8 sectors of 512 bytes
#![allow(unknown_lints, static_mut_refs, dead_code)]

use crate::ahci;
use crate::serial;

const BLOCK: usize = 4096;
const SECTORS_PER_BLOCK: usize = 8;
const MAGIC: &[u8; 8] = b"ASPFS\x01\x00\x00";

// superblock fields
const SB_BLOCK_SIZE: usize = 8;
const SB_TOTAL: usize = 12;
const SB_BITMAP_START: usize = 16;
const SB_BITMAP_BLOCKS: usize = 20;
const SB_ROOT: usize = 24;
const SB_FREE_START: usize = 28;

// directory entry kinds
const K_FREE: u8 = 0;
const K_FILE: u8 = 1;
const K_DIR: u8 = 2;

const MAX_DIRS: usize = 3; // root subdirectories the shell shows (tabs)
const NAME_MAX: usize = 62;

// entry: kind u8, name_len u8, name[62], start u32, blocks u32, size u64
fn entry_kind(blk: &[u8], i: usize) -> u8 {
    blk[8 + i * 128]
}
fn entry_name(blk: &[u8], i: usize, out: &mut [u8]) -> usize {
    let n = blk[8 + i * 128 + 1].min(NAME_MAX as u8) as usize;
    for j in 0..n.min(out.len()) {
        out[j] = blk[8 + i * 128 + 2 + j];
    }
    n
}
fn entry_start(blk: &[u8], i: usize) -> u32 {
    let o = 8 + i * 128 + 64;
    u32::from_le_bytes([blk[o], blk[o + 1], blk[o + 2], blk[o + 3]])
}
fn entry_count(blk: &[u8], i: usize) -> u32 {
    let o = 8 + i * 128 + 68;
    u32::from_le_bytes([blk[o], blk[o + 1], blk[o + 2], blk[o + 3]])
}
fn entry_size(blk: &[u8], i: usize) -> u64 {
    let o = 8 + i * 128 + 72;
    u64::from_le_bytes([
        blk[o], blk[o + 1], blk[o + 2], blk[o + 3], blk[o + 4], blk[o + 5], blk[o + 6], blk[o + 7],
    ])
}
fn set_entry(blk: &mut [u8], i: usize, kind: u8, name: &[u8], start: u32, count: u32, size: u64) {
    let o = 8 + i * 128;
    blk[o..o + 128].fill(0);
    blk[o] = kind;
    blk[o + 1] = name.len().min(NAME_MAX) as u8;
    for j in 0..name.len().min(NAME_MAX) {
        blk[o + 2 + j] = name[j];
    }
    blk[o + 64..o + 68].copy_from_slice(&start.to_le_bytes());
    blk[o + 68..o + 72].copy_from_slice(&count.to_le_bytes());
    blk[o + 72..o + 80].copy_from_slice(&size.to_le_bytes());
}

// cached metadata (all on-disk data the FS touches regularly)
static mut OK: bool = false;
static mut SB: [u8; BLOCK] = [0; BLOCK]; // superblock block
static mut BM: [u8; BLOCK] = [0; BLOCK]; // bitmap block 0 (up to 32768 blocks)
static mut DIRB: [u8; BLOCK] = [0; BLOCK]; // scratch: a directory block
static mut ROOT: [u8; BLOCK] = [0; BLOCK]; // root directory
static mut NDIRS: usize = 0;
static mut DIRNAMES: [[u8; 64]; MAX_DIRS] = [[0; 64]; MAX_DIRS];
static mut DIRBLOCK: [u32; MAX_DIRS] = [0; MAX_DIRS]; // dir block of each subdir

fn blocks() -> u32 {
    unsafe { u32::from_le_bytes(SB[SB_TOTAL..SB_TOTAL + 4].try_into().unwrap()) }
}
fn bitmap_start() -> u32 {
    unsafe { u32::from_le_bytes(SB[SB_BITMAP_START..SB_BITMAP_START + 4].try_into().unwrap()) }
}
fn bitmap_blocks() -> u32 {
    unsafe { u32::from_le_bytes(SB[SB_BITMAP_BLOCKS..SB_BITMAP_BLOCKS + 4].try_into().unwrap()) }
}
fn root_block() -> u32 {
    unsafe { u32::from_le_bytes(SB[SB_ROOT..SB_ROOT + 4].try_into().unwrap()) }
}
fn free_start() -> u32 {
    unsafe { u32::from_le_bytes(SB[SB_FREE_START..SB_FREE_START + 4].try_into().unwrap()) }
}

/// read one 4 KiB block from disk
fn rdb(block: u32, out: &mut [u8]) -> bool {
    let lba = block as u64 * SECTORS_PER_BLOCK as u64;
    ahci::read(lba, SECTORS_PER_BLOCK as u32, out)
}

/// write one 4 KiB block to disk (input must hold BLOCK bytes)
fn wrb(block: u32, data: &[u8]) -> bool {
    let lba = block as u64 * SECTORS_PER_BLOCK as u64;
    ahci::write(lba, SECTORS_PER_BLOCK as u32, data)
}

fn bit_get(n: u32) -> bool {
    unsafe { BM[(n / 8) as usize] & (1 << (n % 8)) != 0 }
}
fn bit_set(n: u32, val: bool) {
    unsafe {
        if val {
            BM[(n / 8) as usize] |= 1 << (n % 8);
        } else {
            BM[(n / 8) as usize] &= !(1 << (n % 8));
        }
    }
}

/// write the cached bitmap back to disk
fn sync_bitmap() -> bool {
    unsafe { wrb(bitmap_start(), &BM) }
}

/// write a directory block back to disk
fn sync_dir(block: u32, blk: &[u8]) -> bool {
    wrb(block, blk)
}

/// find a contiguous free run of `count` blocks, mark it used
fn alloc_contig(count: u32) -> u32 {
    unsafe {
        let mut run = 0u32;
        let mut b = free_start();
        while b < blocks() {
            if bit_get(b) {
                run = 0;
            } else {
                run += 1;
                if run == count {
                    let start = b + 1 - count;
                    for i in start..start + count {
                        bit_set(i, true);
                    }
                    sync_bitmap();
                    return start;
                }
            }
            b += 1;
        }
    }
    0
}

fn free_contig(start: u32, count: u32) {
    for i in start..start + count {
        bit_set(i, false);
    }
    sync_bitmap();
}

/// compare an on-disk name with a wanted name (ascii, case-insensitive)
fn name_eq(disk: &[u8], want: &[u8]) -> bool {
    if disk.len() != want.len() {
        return false;
    }
    for i in 0..want.len() {
        if disk[i].to_ascii_uppercase() != want[i].to_ascii_uppercase() {
            return false;
        }
    }
    true
}

/// mount: read superblock, bitmap and root directory; index subdirectories
pub unsafe fn mount() -> bool {
    OK = false;
    if !ahci::ready() {
        serial::log("fs: no disk\n");
        return false;
    }
    if !rdb(0, &mut SB) {
        serial::log("fs: cannot read superblock\n");
        return false;
    }
    if SB[0..8] != *MAGIC {
        serial::log("fs: bad signature (mkfs image needed)\n");
        return false;
    }
    if u32::from_le_bytes(SB[SB_BLOCK_SIZE..SB_BLOCK_SIZE + 4].try_into().unwrap()) as usize
        != BLOCK
    {
        serial::log("fs: unsupported block size\n");
        return false;
    }
    if !rdb(bitmap_start(), &mut BM) {
        serial::log("fs: cannot read bitmap\n");
        return false;
    }
    if !rdb(root_block(), &mut ROOT) {
        serial::log("fs: cannot read root dir\n");
        return false;
    }

    // index the subdirectories (up to MAX_DIRS)
    NDIRS = 0;
    for i in 0..32usize {
        if ROOT[8 + i * 128] == K_FREE {
            continue;
        }
        if ROOT[8 + i * 128] == K_DIR && NDIRS < MAX_DIRS {
            let mut name = [0u8; 64];
            let n = entry_name(&ROOT, i, &mut name);
            DIRNAMES[NDIRS] = [0u8; 64];
            DIRNAMES[NDIRS][..n].copy_from_slice(&name[..n]);
            DIRBLOCK[NDIRS] = entry_start(&ROOT, i);
            NDIRS += 1;
        }
    }
    OK = true;
    serial::log("fs: ASPFS mounted\n");
    true
}

pub fn ready() -> bool {
    unsafe { OK }
}

pub fn dir_count() -> usize {
    unsafe { NDIRS }
}

/// name of directory `i` into `out`, returns the length
pub fn dir_name(i: usize, out: &mut [u8]) -> usize {
    unsafe {
        if i >= NDIRS {
            return 0;
        }
        let mut n = 0usize;
        while n < 64 && DIRNAMES[i][n] != 0 {
            out[n] = DIRNAMES[i][n];
            n += 1;
        }
        n
    }
}

/// list the files of directory `i` (names, uppercased on disk); returns count
pub fn list_dir(i: usize, names: &mut [[u8; 64]], max: usize) -> usize {
    unsafe {
        if !OK || i >= NDIRS {
            return 0;
        }
        let mut blk = [0u8; BLOCK];
        let mut cur = DIRBLOCK[i];
        let mut n = 0usize;
        loop {
            if cur == 0 || n >= max || !rdb(cur, &mut blk) {
                break;
            }
            for e in 0..32usize {
                if entry_kind(&blk, e) == K_FILE && n < max {
                    let len = entry_name(&blk, e, &mut names[n]);
                    names[n][len] = 0;
                    n += 1;
                }
            }
            cur = u32::from_le_bytes(blk[0..4].try_into().unwrap());
            if cur == DIRBLOCK[i] {
                break; // defensive: no self loops
            }
        }
        n
    }
}

/// find a file entry in directory `i`; returns (dir_block, entry_index, chain_cur)
fn find(dir: usize, name: &[u8]) -> (u32, usize) {
    unsafe {
        let mut blk = [0u8; BLOCK];
        let mut cur = DIRBLOCK[dir];
        while cur != 0 && rdb(cur, &mut blk) {
            for e in 0..32usize {
                // files only: never hand out directory entries as files
                if entry_kind(&blk, e) != K_FILE {
                    continue;
                }
                let mut disk = [0u8; NAME_MAX];
                let n = entry_name(&blk, e, &mut disk) as usize;
                if name_eq(&disk[..n], name) {
                    return (cur, e);
                }
            }
            cur = u32::from_le_bytes(blk[0..4].try_into().unwrap());
        }
    }
    (0, 0)
}

/// read the whole file `name` from directory `dir` into `out`
/// (truncated if the buffer is too small); returns bytes read
pub fn read_file(dir: usize, name: &[u8], out: &mut [u8]) -> usize {
    unsafe {
        if !OK || dir >= NDIRS {
            return 0;
        }
        let (dcur, e) = find(dir, name);
        if dcur == 0 {
            return 0;
        }
        if !rdb(dcur, &mut DIRB) {
            return 0;
        }
        let start = entry_start(&DIRB, e);
        let count = entry_count(&DIRB, e);
        let size = entry_size(&DIRB, e) as usize;
        let cap = out.len().min(size);
        let mut data = [0u8; BLOCK];
        let mut off = 0usize;
        for b in 0..count {
            if off >= cap {
                break;
            }
            if !rdb(start + b, &mut data) {
                return off;
            }
            let n = (cap - off).min(BLOCK);
            out[off..off + n].copy_from_slice(&data[..n]);
            off += n;
        }
        off
    }
}

/// create or overwrite file `name` in directory `dir` with `data`
pub fn write_file(dir: usize, name: &[u8], data: &[u8]) -> bool {
    unsafe {
        if !OK || dir >= NDIRS || name.is_empty() {
            return false;
        }
        let (mut dcur, mut e) = find(dir, name);
        let mut blk = [0u8; BLOCK];

        if dcur != 0 {
            // existing file: reuse its extent when the data fits
            rdb(dcur, &mut blk);
            let old_start = entry_start(&blk, e);
            let old_count = entry_count(&blk, e);
            let need = ((data.len() + BLOCK - 1) / BLOCK).max(1) as u32;
            let (start, count) = if need <= old_count {
                (old_start, old_count)
            } else {
                free_contig(old_start, old_count);
                let s = alloc_contig(need);
                if s == 0 {
                    return false;
                }
                (s, need)
            };
            let mut chunk = [0u8; BLOCK];
            let mut off = 0usize;
            for b in 0..count {
                let end = (off + BLOCK).min(data.len());
                for i in 0..BLOCK {
                    chunk[i] = if off + i < end { data[off + i] } else { 0 };
                }
                if !wrb(start + b, &chunk) {
                    return false;
                }
                off += BLOCK;
            }
            set_entry(&mut blk, e, K_FILE, name, start, count, data.len() as u64);
            sync_dir(dcur, &blk);
            return true;
        }

        // new file: claim a free entry in the directory chain
        let need = ((data.len() + BLOCK - 1) / BLOCK).max(1) as u32;
        let start = alloc_contig(need);
        if start == 0 {
            return false;
        }
        let mut placed = false;
        let mut chain = DIRBLOCK[dir];
        let mut chain_new = 0u32;
        while chain != 0 && rdb(chain, &mut blk) {
            for i in 0..32usize {
                if entry_kind(&blk, i) == K_FREE {
                    set_entry(&mut blk, i, K_FILE, name, start, need, data.len() as u64);
                    sync_dir(chain, &blk);
                    placed = true;
                    break;
                }
            }
            if placed {
                break;
            }
            chain_new = u32::from_le_bytes(blk[0..4].try_into().unwrap());
            if chain_new == 0 {
                // chain full: extend it with a new directory block
                let nb = alloc_contig(1);
                if nb == 0 {
                    return false;
                }
                let mut nbbuf = [0u8; BLOCK];
                nbbuf[0..4].copy_from_slice(&0u32.to_le_bytes());
                nbbuf[4..8].copy_from_slice(&0u32.to_le_bytes());
                sync_dir(nb, &nbbuf);
                blk[0..4].copy_from_slice(&nb.to_le_bytes());
                sync_dir(chain, &blk);
                chain_new = nb;
            }
            chain = chain_new;
        }
        if !placed {
            return false;
        }

        // write the file data blocks
        let mut chunk = [0u8; BLOCK];
        let mut off = 0usize;
        for b in 0..need {
            let end = (off + BLOCK).min(data.len());
            for i in 0..BLOCK {
                chunk[i] = if off + i < end { data[off + i] } else { 0 };
            }
            if !wrb(start + b, &chunk) {
                return false;
            }
            off += BLOCK;
        }
        true
    }
}

/// delete file `name` from directory `dir` (frees the extent + entry)
pub fn delete_file(dir: usize, name: &[u8]) -> bool {
    unsafe {
        if !OK || dir >= NDIRS {
            return false;
        }
        let (dcur, e) = find(dir, name);
        if dcur == 0 {
            return false;
        }
        let mut blk = [0u8; BLOCK];
        if !rdb(dcur, &mut blk) {
            return false;
        }
        let start = entry_start(&blk, e);
        let count = entry_count(&blk, e);
        if count > 0 {
            free_contig(start, count);
        }
        blk[8 + e * 128] = K_FREE;
        blk[8 + e * 128 + 1] = 0;
        sync_dir(dcur, &blk);
        true
    }
}

/// free disk space in KiB (from the allocation bitmap)
pub fn free_kb() -> u32 {
    unsafe {
        if !OK {
            return 0;
        }
        let mut free = 0u32;
        let mut b = free_start();
        while b < blocks() {
            if !bit_get(b) {
                free += 1;
            }
            b += 1;
        }
        free * 4
    }
}
