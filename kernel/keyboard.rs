// ASPECT OS - PS/2 keyboard: scancode set 2 -> key codes
// code: 1..=255 = ASCII char, 256 = Enter, 257 = Backspace, 258 = Escape, 0 = ignore
use crate::io::inb;
use core::sync::atomic::{AtomicBool, Ordering};

static SHIFT: AtomicBool = AtomicBool::new(false);
static EXTENDED: AtomicBool = AtomicBool::new(false);

pub const K_ENTER: u32 = 256;
pub const K_BACKSPACE: u32 = 257;
pub const K_ESCAPE: u32 = 258;
pub const K_UP: u32 = 259;
pub const K_DOWN: u32 = 260;
pub const K_LEFT: u32 = 261;
pub const K_RIGHT: u32 = 262;

// scancode set 2 make codes -> ASCII (lowercase base mapping)
const KEYMAP: [(u8, u8); 51] = [
    // letters
    (0x1C, b'a'), (0x32, b'b'), (0x21, b'c'), (0x23, b'd'), (0x24, b'e'),
    (0x2B, b'f'), (0x34, b'g'), (0x33, b'h'), (0x43, b'i'), (0x3B, b'j'),
    (0x42, b'k'), (0x4B, b'l'), (0x3A, b'm'), (0x31, b'n'), (0x44, b'o'),
    (0x4D, b'p'), (0x15, b'q'), (0x2D, b'r'), (0x1B, b's'), (0x2C, b't'),
    (0x3C, b'u'), (0x2A, b'v'), (0x1D, b'w'), (0x22, b'x'), (0x35, b'y'),
    (0x1A, b'z'),
    // digits
    (0x16, b'1'), (0x1E, b'2'), (0x26, b'3'), (0x25, b'4'), (0x2E, b'5'),
    (0x36, b'6'), (0x3D, b'7'), (0x3E, b'8'), (0x46, b'9'), (0x45, b'0'),
    // punctuation (unshifted)
    (0x29, b' '), (0x4E, b'-'), (0x55, b'='), (0x41, b','), (0x49, b'.'),
    (0x4A, b'/'), (0x4C, b';'), (0x52, b'\''), (0x5B, b'\\'), (0x54, b'['),
    (0x5D, b']'), (0x0E, b'`'),
    // control keys
    (0x5A, b'\n'),   // Enter
    (0x66, 0x08),    // Backspace
    (0x76, 0x1B),    // Escape
];

// shifted variants (same make codes -> shifted character)
const SHIFTMAP: [(u8, u8); 21] = [
    (0x16, b'!'), (0x1E, b'@'), (0x26, b'#'), (0x25, b'$'),
    (0x2E, b'%'), (0x36, b'^'), (0x3D, b'&'), (0x3E, b'*'),
    (0x46, b'('), (0x45, b')'),
    (0x4C, b':'), (0x52, b'"'), (0x5B, b'|'), (0x54, b'{'), (0x5D, b'}'),
    (0x0E, b'~'),
    (0x4E, b'_'), (0x55, b'+'), (0x41, b'<'), (0x49, b'>'), (0x4A, b'?'),
];

fn lookup(table: &[(u8, u8)], make: u8) -> Option<u8> {
    for (sc, ch) in table {
        if *sc == make {
            return Some(*ch);
        }
    }
    None
}

fn to_code(ch: u8) -> u32 {
    match ch {
        b'\n' => K_ENTER,
        0x08 => K_BACKSPACE,
        0x1B => K_ESCAPE,
        c => c as u32,
    }
}

fn translate(make: u8, shift: bool) -> u32 {
    // shift keys are handled separately
    if make == 0x12 || make == 0x59 {
        return 0;
    }
    if shift {
        if let Some(ch) = lookup(&SHIFTMAP, make) {
            return ch as u32;
        }
    }
    if let Some(ch) = lookup(&KEYMAP, make) {
        if ch == b'\n' || ch == 0x08 || ch == 0x1B {
            return to_code(ch);
        }
        if ch.is_ascii_lowercase() && shift {
            return (ch - 32) as u32;
        }
        return ch as u32;
    }
    0
}

static mut LAST_MAKE: u8 = 0;
static mut LAST_TICKS: u64 = 0;

/// called from the IRQ1 stub; returns 0 or a key code
pub unsafe fn handle() -> u32 {
    let sc = inb(0x60);

    let make = sc & 0x7F;
    let press = sc & 0x80 == 0;

    // E0 prefix: extended keys (arrows etc.)
    if sc == 0xE0 {
        EXTENDED.store(true, Ordering::Relaxed);
        return 0;
    }
    if EXTENDED.swap(false, Ordering::Relaxed) {
        if !press {
            return 0;
        }
        // translated extended make codes (scancode set 1 style)
        return match make {
            0x48 => K_UP,
            0x50 => K_DOWN,
            0x4B => K_LEFT,
            0x4D => K_RIGHT,
            _ => 0,
        };
    }

    if make == 0x12 || make == 0x59 {
        SHIFT.store(press, Ordering::Relaxed);
        return 0;
    }
    if !press {
        return 0;
    }
    // debounce: QEMU (and some keyboards) can deliver a make twice within
    // a few milliseconds; drop an identical make inside 20 ticks (20 ms)
    let now = crate::pit::ticks();
    if make == LAST_MAKE && now.wrapping_sub(LAST_TICKS) < 20 {
        return 0;
    }
    LAST_MAKE = make;
    LAST_TICKS = now;
    translate(make, SHIFT.load(Ordering::Relaxed))
}
