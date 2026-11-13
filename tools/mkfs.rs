// ASPECT OS - mkfs: creates a raw data disk image with the ASPFS file
// system (own format). Runs on the host, writes build/aspect-data.img.
//
// ASPFS layout (4 KiB blocks, 512-byte sectors underneath):
//   block 0            superblock
//   blocks 1..n        block allocation bitmap (1 bit per block)
//   after the bitmap   directory blocks and file data
//
// superblock (block 0):
//   0  magic "ASPFS" + version u8 + pad
//   8  u32 block_size      4096
//   12 u32 total_blocks
//   16 u32 bitmap_start    1
//   20 u32 bitmap_blocks
//   24 u32 root_block
//   28 u32 free_start      first allocatable block
//   32 u32 label_len, label bytes (unused, kept zero)
//
// directory block:
//   0 u32 next_dir_block (0 = last)
//   4 u32 entry_count (entries in this block)
//   8 entry[32] of 128 bytes:
//     +0  u8  kind (0 free, 1 file, 2 dir)
//     +1  u8  name_len
//     +2  u8  name[62] (uppercase ascii)
//     +64 u32 start_block
//     +68 u32 block_count
//     +72 u64 size
//
// files are contiguous extents (start_block + block_count); this keeps the
// kernel side simple and is plenty for text files on a small disk.
//
// usage: mkfs <image-path> [size-mib]

use std::env;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::process::exit;

const BLOCK: usize = 4096;
const MAGIC: &[u8; 8] = b"ASPFS\x01\x00\x00";

struct Img {
    path: String,
    blocks: u32,
    bitmap: Vec<u8>, // 1 bit per block
    root: u32,
    free_start: u32,
}

impl Img {
    fn alloc(&mut self, count: u32) -> Option<u32> {
        let mut run = 0u32;
        let mut b = self.free_start;
        while b < self.blocks {
            let used = self.bitmap[(b / 8) as usize] & (1 << (b % 8)) != 0;
            if used {
                run = 0;
            } else {
                run += 1;
                if run == count {
                    let start = b + 1 - count;
                    for i in start..start + count {
                        self.bitmap[(i / 8) as usize] |= 1 << (i % 8);
                    }
                    return Some(start);
                }
            }
            b += 1;
        }
        None
    }

    fn write_block(&self, f: &mut File, block: u32, data: &[u8]) {
        assert!(data.len() <= BLOCK, "block overflow");
        f.seek(SeekFrom::Start(block as u64 * BLOCK as u64)).unwrap();
        let mut buf = vec![0u8; BLOCK];
        buf[..data.len()].copy_from_slice(data);
        f.write_all(&buf).unwrap();
    }
}

/// name for dir entries: uppercase ascii, max 62 bytes
fn clean_name(name: &str) -> Vec<u8> {
    let mut v: Vec<u8> = name.to_ascii_uppercase().into_bytes();
    v.retain(|&b| b >= 32 && b < 127 && b != b'/' && b != b'\\');
    v.truncate(62);
    v
}

/// build one directory block from (kind, name, start, count, size) tuples
fn dir_block(next: u32, entries: &[(u8, Vec<u8>, u32, u32, u64)]) -> Vec<u8> {
    let mut blk = vec![0u8; BLOCK];
    blk[0..4].copy_from_slice(&next.to_le_bytes());
    blk[4..8].copy_from_slice(&(entries.len() as u32).to_le_bytes());
    let mut off = 8usize;
    for (kind, name, start, count, size) in entries {
        blk[off] = *kind;
        blk[off + 1] = name.len().min(62) as u8;
        blk[off + 2..off + 2 + name.len().min(62)]
            .copy_from_slice(&name[..name.len().min(62)]);
        blk[off + 64..off + 68].copy_from_slice(&start.to_le_bytes());
        blk[off + 68..off + 72].copy_from_slice(&count.to_le_bytes());
        blk[off + 72..off + 80].copy_from_slice(&size.to_le_bytes());
        off += 128;
    }
    blk
}

/// create a file in the image; returns (start_block, block_count)
fn put_file(img: &mut Img, f: &mut File, data: &[u8]) -> (u32, u32) {
    let mut count = (data.len() + BLOCK - 1) / BLOCK;
    if count == 0 {
        count = 1;
    }
    let count32 = count as u32;
    let start = img.alloc(count32).expect("disk image too small");
    let mut off = 0usize;
    for b in 0..count {
        let end = (off + BLOCK).min(data.len());
        let chunk = if off < end { &data[off..end] } else { &[][..] };
        let mut blk = vec![0u8; chunk.len()];
        blk.copy_from_slice(chunk);
        img.write_block(f, start + b as u32, &blk);
        off += BLOCK;
    }
    (start, count as u32)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: mkfs <image-path> [size-mib]");
        exit(1);
    }
    let path = args[1].clone();
    let mib: u64 = if args.len() > 2 {
        args[2].parse().unwrap_or(16)
    } else {
        16
    };

    let blocks = (mib * 1024 * 1024 / BLOCK as u64) as u32;
    let bitmap_blocks = (blocks + BLOCK as u32 * 8 - 1) / (BLOCK as u32 * 8);
    let free_start = 1 + bitmap_blocks;

    let mut img = Img {
        path: path.clone(),
        blocks,
        bitmap: vec![0; (blocks as usize + 7) / 8],
        root: 0,
        free_start,
    };

    // reserve superblock + bitmap blocks in the allocation bitmap
    for b in 0..free_start {
        img.bitmap[(b / 8) as usize] |= 1 << (b % 8);
    }

    let mut f = File::create(&path).unwrap();
    f.set_len(mib * 1024 * 1024).unwrap();

    // --- directories (SYSTEM, USERS, GUEST) ---
    let sys_blk = img.alloc(1).unwrap();
    let usr_blk = img.alloc(1).unwrap();
    let gst_blk = img.alloc(1).unwrap();
    let root = img.alloc(1).unwrap();
    img.root = root;

    // root dir with three subdirectories
    let entries = vec![
        (2u8, clean_name("SYSTEM"), sys_blk, 1u32, 0u64),
        (2u8, clean_name("USERS"), usr_blk, 1u32, 0u64),
        (2u8, clean_name("GUEST"), gst_blk, 1u32, 0u64),
    ];
    let blk = dir_block(0, &entries);
    img.write_block(&mut f, root, &blk);
    img.write_block(&mut f, sys_blk, &dir_block(0, &[]));
    img.write_block(&mut f, usr_blk, &dir_block(0, &[]));
    img.write_block(&mut f, gst_blk, &dir_block(0, &[]));

    // --- seed files: same content the static VFS used to show ---
    let system: [(&str, &str); 3] = [
        (
            "WELCOME.TXT",
            "WELCOME TO ASPECT OS\n\nTHIS SYSTEM IS BUILT\nFROM SCRATCH IN RUST:\nOWN BOOTLOADER, OWN\n64-BIT KERNEL, OWN\nGUI AND APPS.",
        ),
        (
            "ASPECT.SYS",
            "ASPECT.SYS CORE\n\nKERNEL: RUST, NO STD\nPMM: BITMAP ALLOCATOR\nINPUT: PS/2 KEYBOARD\nAND MOUSE, OWN DRIVERS\nVIDEO: GOP FRAMEBUFFER\nDISK: OWN PCI + AHCI\nFS: ASPFS (M5)",
        ),
        (
            "ROADMAP.TXT",
            "ROADMAP\n\nM1 BOOT + VIDEO OK\nM2 INTERRUPTS OK\nM3 INPUT + PMM OK\nM4 WINDOWS + APPS\nM5 REAL FILE SYSTEM\nM6 NETWORK STACK",
        ),
    ];
    let users: [(&str, &str); 3] = [
        (
            "KLEMENS.TXT",
            "HOME FOLDER\n\nTHIS IS THE USER\nFOLDER OF THE ONLY\nUSER OF THIS MACHINE.",
        ),
        (
            "NOTES.TXT",
            "NOTES\n\nREMEMBER: NO GRUB,\nNO GNU EFI, NO LINUX.\nEVERYTHING OURS.",
        ),
        (
            "PROJECTS.TXT",
            "PROJECTS\n\n1. WINDOW COMPOSITOR\n2. PAINT APP\n3. OFFLINE BROWSER\n4. REAL FILE SYSTEM",
        ),
    ];
    let guest: [(&str, &str); 3] = [
        (
            "GUEST.LOG",
            "GUEST SESSION LOG\n\nLOGIN AT BOOT TIME\nNO FILES MODIFIED",
        ),
        (
            "TEMP.TXT",
            "TEMP.TXT\n\nTHIS FILE IS SAFE\nTO DELETE. THE FS\nIS ON REAL DISK NOW\n(ASPFS OVER AHCI).",
        ),
        (
            "README.TXT",
            "README.TXT\n\nTHE GUEST FOLDER IS\nFOR VISITORS AND\nTEST DOCUMENTS.",
        ),
    ];
    for (dir_blk, set) in [
        (sys_blk, &system),
        (usr_blk, &users),
        (gst_blk, &guest),
    ] {
        let mut entries = Vec::new();
        for (name, text) in set {
            let (start, count) = put_file(&mut img, &mut f, text.as_bytes());
            entries.push((
                1u8,
                clean_name(name),
                start,
                count,
                text.len() as u64,
            ));
        }
        let blk = dir_block(0, &entries);
        img.write_block(&mut f, dir_blk, &blk);
    }

    // --- superblock + bitmap last (they know all allocations) ---
    let mut sb = vec![0u8; BLOCK];
    sb[0..8].copy_from_slice(MAGIC);
    sb[8..12].copy_from_slice(&(BLOCK as u32).to_le_bytes());
    sb[12..16].copy_from_slice(&blocks.to_le_bytes());
    sb[16..20].copy_from_slice(&1u32.to_le_bytes());
    sb[20..24].copy_from_slice(&bitmap_blocks.to_le_bytes());
    sb[24..28].copy_from_slice(&root.to_le_bytes());
    sb[28..32].copy_from_slice(&free_start.to_le_bytes());
    img.write_block(&mut f, 0, &sb);

    let mut bmbytes = vec![0u8; bitmap_blocks as usize * BLOCK];
    bmbytes[..img.bitmap.len()].copy_from_slice(&img.bitmap);
    for b in 0..bitmap_blocks {
        let s = (b as usize * BLOCK).min(bmbytes.len());
        let e = ((b as usize + 1) * BLOCK).min(bmbytes.len());
        img.write_block(&mut f, 1 + b, &bmbytes[s..e]);
    }
    f.sync_all().unwrap();

    println!(
        "mkfs: {} - {} blocks ({} KiB), bitmap {} block(s), root at block {}",
        path, blocks, mib * 1024, bitmap_blocks, root
    );
}
