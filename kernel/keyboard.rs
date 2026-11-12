// ASPECT OS - PS/2 keyboard: scancode set 2 -> key events
use crate::io::inb;
use core::sync::atomic::{AtomicBool, Ordering};

static SHIFT: AtomicBool = AtomicBool::new(false);
static EXTENDED: AtomicBool = AtomicBool::new(false);

// scancode set 2 make codes -> ASCII (lowercase)
const KEYMAP: [(u8, u8); 40] = [
    (0x1C, b'a'), (0x32, b'b'), (0x21, b'c'), (0x23, b'd'), (0x24, b'e'),
    (0x2B, b'f'), (0x34, b'g'), (0x33, b'h'), (0x43, b'i'), (0x3B, b'j'),
    (0x42, b'k'), (0x4B, b'l'), (0x3A, b'm'), (0x31, b'n'), (0x44, b'o'),
    (0x4D, b'p'), (0x15, b'q'), (0x2D, b'r'), (0x1B, b's'), (0x2C, b't'),
    (0x3C, b'u'), (0x2A, b'v'), (0x1D, b'w'), (0x22, b'x'), (0x35, b'y'),
    (0x26, b'z'),
    (0x16, b'1'), (0x1E, b'2'), (0x25, b'3'), (0x25, b'4'), (0x2E, b'5'),
    (0x36, b'6'), (0x3D, b'7'), (0x3E, b'8'), (0x46, b'9'), (0x45, b'0'),
    (0x29, b' '), (0x5A, b'\n'), (0x66, 0x08), (0x76, 0x1B), // Space, Enter, Backspace, Esc
];

pub enum KeyEvent {
    Char(u8),
    Enter,
    Backspace,
    Escape,
    Ignore,
}

fn translate(make: u8, shift: bool) -> KeyEvent {
    // shift keys are handled separately
    if make == 0x12 || make == 0x59 {
        return KeyEvent::Ignore;
    }
    for (sc, ch) in KEYMAP {
        if sc == make {
            if ch == b'\n' {
                return KeyEvent::Enter;
            }
            if ch == 0x08 {
                return KeyEvent::Backspace;
            }
            if ch == 0x1B {
                return KeyEvent::Escape;
            }
            let c = if shift && ch.is_ascii_lowercase() {
                ch - 32
            } else {
                ch
            };
            return KeyEvent::Char(c);
        }
    }
    KeyEvent::Ignore
}

/// called from the IRQ1 stub; returns the translated event
pub unsafe fn handle() -> KeyEvent {
    let sc = inb(0x60);

    // E0 prefix: extended keys (arrows etc.) -> ignore for now
    if sc == 0xE0 {
        EXTENDED.store(true, Ordering::Relaxed);
        return KeyEvent::Ignore;
    }
    if EXTENDED.swap(false, Ordering::Relaxed) {
        return KeyEvent::Ignore;
    }

    let make = sc & 0x7F;
    let press = sc & 0x80 == 0;

    if make == 0x12 || make == 0x59 {
        SHIFT.store(press, Ordering::Relaxed);
        return KeyEvent::Ignore;
    }
    if !press {
        return KeyEvent::Ignore;
    }
    translate(make, SHIFT.load(Ordering::Relaxed))
}
