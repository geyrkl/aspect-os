// ASPECT OS - PS/2 mouse: 3-byte packets -> cursor deltas + buttons

// IRQ context: no concurrent access, static mut is safe here
static mut PKT: [u8; 3] = [0; 3];
static mut IDX: u8 = 0;
static mut BTN: u8 = 0;

/// called from the IRQ12 stub with the data byte. Returns Some(event) only
/// when a full packet is decoded - lead bytes and stray bytes return None
/// so the queue never sees empty filler events between real packets.
pub unsafe fn handle(b: u8) -> Option<(i32, i32, u8, bool)> {
    let idx = IDX;

    // byte 0: sync bit 3 must be set and bits 6/7 must be clear
    // (real packet starts are 0x08..0x2F; stray bytes like 0xFA are discarded)
    if idx == 0 && (b & 0xC8) != 0x08 {
        return None;
    }

    PKT[idx as usize] = b;
    IDX = idx + 1;
    if IDX < 3 {
        return None;
    }
    IDX = 0;

    let b0 = PKT[0];
    // 9-bit two's complement deltas: 8 value bits + sign bit in byte 0
    let mut dx = PKT[1] as i32;
    let mut dy = PKT[2] as i32;
    let buttons = b0 & 0x07;

    if b0 & 0x10 != 0 {
        dx -= 256;
    }
    if b0 & 0x20 != 0 {
        dy -= 256;
    }
    // mouse y points up -> screen y points down
    dy = -dy;

    let prev = BTN;
    BTN = buttons;
    let click = (buttons & 1) != 0 && (prev & 1) == 0;

    Some((dx, dy, buttons, click))
}
