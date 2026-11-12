// ASPECT OS - own GUI library in Rust: draws directly to the framebuffer
// Gradient, menubar, dock, clock, cursor - all our own code.
use crate::bootinfo::BootInfo;
use crate::font;

static mut FB: *mut u32 = core::ptr::null_mut();
static mut W: i32 = 0;
static mut H: i32 = 0;
static mut PPSL: i32 = 0;

pub fn fb_init(bi: &BootInfo) {
    unsafe {
        FB = bi.fb_base as *mut u32;
        W = bi.fb_width as i32;
        H = bi.fb_height as i32;
        PPSL = bi.fb_pitch_px as i32;
    }
}

pub fn width() -> i32 {
    unsafe { W }
}
pub fn height() -> i32 {
    unsafe { H }
}

// colors are 0xAARRGGBB
#[inline]
pub fn mix(d: u32, s: u32, a: i32) -> u32 {
    let dr = (d & 255) as i32;
    let dg = ((d >> 8) & 255) as i32;
    let db = ((d >> 16) & 255) as i32;
    let sr = (s & 255) as i32;
    let sg = ((s >> 8) & 255) as i32;
    let sb = ((s >> 16) & 255) as i32;
    let r = dr + (((sr - dr) * a) >> 8);
    let g = dg + (((sg - dg) * a) >> 8);
    let b = db + (((sb - db) * a) >> 8);
    0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

fn px(x: i32, y: i32, c: u32) {
    unsafe {
        if x < 0 || y < 0 || x >= W || y >= H {
            return;
        }
        *FB.offset((y * PPSL + x) as isize) = c;
    }
}

pub fn px_blend(x: i32, y: i32, c: u32, a: i32) {
    unsafe {
        if x < 0 || y < 0 || x >= W || y >= H {
            return;
        }
        let p = FB.offset((y * PPSL + x) as isize);
        *p = mix(*p, c, a);
    }
}

#[inline]
fn clamp(v: i32, lo: i32, hi: i32) -> i32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

fn fill_rect(x0: i32, y0: i32, w: i32, h: i32, c: u32) {
    unsafe {
        let xa = clamp(x0, 0, W - 1);
        let xb = clamp(x0 + w - 1, 0, W - 1);
        let ya = clamp(y0, 0, H - 1);
        let yb = clamp(y0 + h - 1, 0, H - 1);
        for y in ya..=yb {
            let row = FB.offset((y * PPSL) as isize);
            for x in xa..=xb {
                *row.offset(x as isize) = c;
            }
        }
    }
}

fn blend_rect(x0: i32, y0: i32, w: i32, h: i32, c: u32, a: i32) {
    unsafe {
        let xa = clamp(x0, 0, W - 1);
        let xb = clamp(x0 + w - 1, 0, W - 1);
        let ya = clamp(y0, 0, H - 1);
        let yb = clamp(y0 + h - 1, 0, H - 1);
        for y in ya..=yb {
            let row = FB.offset((y * PPSL) as isize);
            for x in xa..=xb {
                let p = row.offset(x as isize);
                *p = mix(*p, c, a);
            }
        }
    }
}

pub fn isqrt(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = 1u64;
    while x > y {
        x = (x + y) >> 1;
        y = n / x;
    }
    x
}

// scanline extents of a rounded rect in row y (0-based within the rect)
fn rr_row(h: i32, r: i32, y: i32) -> i32 {
    if y < r {
        isqrt((r * r - (r - 1 - y) * (r - 1 - y)) as u64) as i32
    } else if y >= h - r {
        isqrt((r * r - (y - (h - r)) * (y - (h - r))) as u64) as i32
    } else {
        r
    }
}

fn round_rect_fill(x0: i32, y0: i32, w: i32, h: i32, mut r: i32, c: u32) {
    if r > h / 2 {
        r = h / 2;
    }
    if r > w / 2 {
        r = w / 2;
    }
    for y in 0..h {
        let d = rr_row(h, r, y);
        for x in x0 + r - d..=x0 + w - 1 - r + d {
            px(x, y0 + y, c);
        }
    }
}

fn round_rect_blend(x0: i32, y0: i32, w: i32, h: i32, mut r: i32, c: u32, a: i32) {
    if r > h / 2 {
        r = h / 2;
    }
    if r > w / 2 {
        r = w / 2;
    }
    for y in 0..h {
        let d = rr_row(h, r, y);
        for x in x0 + r - d..=x0 + w - 1 - r + d {
            px_blend(x, y0 + y, c, a);
        }
    }
}

// rounded rect outline (signed distance, integer math only)
fn round_rect_stroke(x0: i32, y0: i32, w: i32, h: i32, mut r: i32, t: i32, c: u32) {
    if r > h / 2 - t {
        r = h / 2 - t;
    }
    if r > w / 2 - t {
        r = w / 2 - t;
    }
    if r < 1 {
        fill_rect(x0, y0, w, t, c);
        fill_rect(x0, y0 + h - t, w, t, c);
        fill_rect(x0, y0, t, h, c);
        fill_rect(x0 + w - t, y0, t, h, c);
        return;
    }
    let hw = w / 2;
    let hh = h / 2;
    let cyy = y0 + hh;
    let cxx = x0 + hw;
    for y in y0..y0 + h {
        let dy = y - cyy;
        for x in x0..x0 + w {
            let dxx = x - cxx;
            let mut qx = dxx.abs() - (hw - r);
            if qx < 0 {
                qx = 0;
            }
            let mut qy = dy.abs() - (hh - r);
            if qy < 0 {
                qy = 0;
            }
            let dmin = qx.min(qy).max(0);
            let d = dmin + isqrt((qx * qx + qy * qy) as u64) as i32 - r;
            if d.abs() <= t / 2 {
                px(x, y, c);
            }
        }
    }
}

// thick line (Bresenham + square brush)
pub fn draw_line_t(x0: i32, y0: i32, x1: i32, y1: i32, t: i32, c: u32) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let adx = dx.abs();
    let ady = dy.abs();
    let sx = if dx > 0 { 1 } else { -1 };
    let sy = if dy > 0 { 1 } else { -1 };
    let mut err = adx - ady; // classic Bresenham initial error
    let mut x = x0;
    let mut y = y0;
    let half = t / 2;
    loop {
        for oy in -half..=half {
            for ox in -half..=half {
                px(x + ox, y + oy, c);
            }
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = err * 2;
        if e2 > -ady {
            err -= ady;
            x += sx;
        }
        if e2 < adx {
            err += adx;
            y += sy;
        }
    }
}

// fully filled circle
fn circle_fill(cx: i32, cy: i32, r: i32, c: u32) {
    for y in -r..=r {
        for x in -r..=r {
            if x * x + y * y <= r * r {
                px(cx + x, cy + y, c);
            }
        }
    }
}

// circle ring
fn ring_circle(cx: i32, cy: i32, r: i32, t: i32, c: u32) {
    for y in -(r + t)..=(r + t) {
        for x in -(r + t)..=(r + t) {
            let d = isqrt((x * x + y * y) as u64) as i32;
            if d >= r - t / 2 - 1 && d <= r + t / 2 {
                px(cx + x, cy + y, c);
            }
        }
    }
}

// wifi arc (cone pointing up)
fn wifi_arc(cx: i32, cy: i32, r: i32, c: u32) {
    for y in -(r + 2)..=(r + 2) {
        for x in -(r + 2)..=(r + 2) {
            let d = isqrt((x * x + y * y) as u64) as i32;
            if d >= r - 2 && d <= r + 2 && y < 0 && x.abs() * 7 <= (-y) * 4 {
                px(cx + x, cy + y, c);
            }
        }
    }
}

// --- desktop colors (LiquidGlass reference gradient) ---
const C00: u32 = 0xFF29_2D63; // top left
const C10: u32 = 0xFF6B_317F; // top right
const C01: u32 = 0xFF59_2D6D; // bottom left
const C11: u32 = 0xFF94_2E7D; // bottom right

/// gradient color at a single pixel (same formula as draw_gradient)
pub fn gradient_color(x: i32, y: i32) -> u32 {
    unsafe {
        let w = W;
        let h = H;
        let u = ((x as i64) * 256 / ((w - 1).max(1) as i64)) as i32;
        let v = ((y as i64) * 256 / ((h - 1).max(1) as i64)) as i32;
        let w00 = (256 - u) * (256 - v);
        let w10 = u * (256 - v);
        let w01 = (256 - u) * v;
        let w11 = u * v;
        let r = (w00 * ((C00 >> 16) & 255) as i32
            + w10 * ((C10 >> 16) & 255) as i32
            + w01 * ((C01 >> 16) & 255) as i32
            + w11 * ((C11 >> 16) & 255) as i32)
            >> 16;
        let g = (w00 * ((C00 >> 8) & 255) as i32
            + w10 * ((C10 >> 8) & 255) as i32
            + w01 * ((C01 >> 8) & 255) as i32
            + w11 * ((C11 >> 8) & 255) as i32)
            >> 16;
        let b = (w00 * (C00 & 255) as i32
            + w10 * (C10 & 255) as i32
            + w01 * (C01 & 255) as i32
            + w11 * (C11 & 255) as i32)
            >> 16;
        0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | b as u32
    }
}

pub fn draw_gradient() {
    unsafe {
        let (w, h) = (W, H);
        for y in 0..h {
            let v = ((y as i64) * 256 / (h - 1).max(1) as i64) as i32;
            let row = FB.offset((y * PPSL) as isize);
            for x in 0..w {
                let u = ((x as i64) * 256 / (w - 1).max(1) as i64) as i32;
                let w00 = (256 - u) * (256 - v);
                let w10 = u * (256 - v);
                let w01 = (256 - u) * v;
                let w11 = u * v;
                let r = (w00 * ((C00 >> 16) & 255) as i32
                    + w10 * ((C10 >> 16) & 255) as i32
                    + w01 * ((C01 >> 16) & 255) as i32
                    + w11 * ((C11 >> 16) & 255) as i32)
                    >> 16;
                let g = (w00 * ((C00 >> 8) & 255) as i32
                    + w10 * ((C10 >> 8) & 255) as i32
                    + w01 * ((C01 >> 8) & 255) as i32
                    + w11 * ((C11 >> 8) & 255) as i32)
                    >> 16;
                let b = (w00 * (C00 & 255) as i32
                    + w10 * (C10 & 255) as i32
                    + w01 * (C01 & 255) as i32
                    + w11 * (C11 & 255) as i32)
                    >> 16;
                *row.offset(x as isize) =
                    0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | b as u32;
            }
        }
    }
}

pub fn draw_menubar() {
    let w = width();
    blend_rect(0, 0, w, 28, 0x0A0A0C, 150);
    blend_rect(0, 27, w, 1, 0xFFFFFF, 30);

    // monogram on the left: stylized "A"
    draw_line_t(14, 21, 18, 7, 2, 0xFFB9A8F5);
    draw_line_t(18, 7, 22, 21, 2, 0xFFF5A8E8);
    draw_line_t(16, 15, 20, 15, 2, 0xFFFFFFFF);

    // right side: wifi + battery
    let wx = w - 96;
    wifi_arc(wx, 24, 9, 0xFFE8E8EC);
    wifi_arc(wx, 24, 5, 0xFFE8E8EC);
    fill_rect(wx - 1, 20, 3, 3, 0xFFE8E8EC);
    let bx = w - 66;
    round_rect_stroke(bx, 9, 26, 13, 3, 2, 0xFFC8C8CC);
    fill_rect(bx + 3, 12, 15, 7, 0xFFE8E8EC);
    fill_rect(bx + 27, 12, 2, 7, 0xFFC8C8CC);

    draw_clock(0);
}

// --- clock in the menubar (HH:MM:SS, bitmap digits) ---
const CLOCK_SCALE: i32 = 2;
const GLYPH_ADV: i32 = (font::GLYPH_W as i32 + 2) * CLOCK_SCALE; // advance per glyph

fn draw_glyph(g: &[u8; 7], x0: i32, y0: i32, c: u32) {
    for (row, bits) in g.iter().enumerate() {
        for col in 0..font::GLYPH_W {
            if bits & (0x8 >> col) != 0 {
                for oy in 0..CLOCK_SCALE {
                    for ox in 0..CLOCK_SCALE {
                        px(x0 + (col as i32) * CLOCK_SCALE + ox,
                           y0 + (row as i32) * CLOCK_SCALE + oy, c);
                    }
                }
            }
        }
    }
}

/// secs = seconds since boot; restore the background (menubar over gradient)
pub fn draw_clock(secs: u64) {
    let h = ((secs / 3600) % 24) as usize;
    let m = ((secs / 60) % 60) as usize;
    let s = (secs % 60) as usize;

    let text_w = 8 * GLYPH_ADV - 4 * CLOCK_SCALE;
    let x0 = width() - 96 - 8 - text_w; // left of wifi/battery
    let y0 = 7;

    // repaint the clock area: menubar color = gradient + dark blend
    for y in y0 - 1..y0 + (font::GLYPH_H as i32) * CLOCK_SCALE + 1 {
        for x in x0 - 2..x0 + text_w + 2 {
            let base = mix(gradient_color(x, y), 0x0A0A0C, 150);
            px(x, y, base);
        }
    }

    // H H : M M : S S
    let seq: [(usize, bool); 8] = [
        (h / 10, false), (h % 10, false), (0, true),
        (m / 10, false), (m % 10, false), (0, true),
        (s / 10, false), (s % 10, false),
    ];
    for (i, (d, colon)) in seq.iter().enumerate() {
        let glyph: &[u8; 7] = if *colon { &font::COLON } else { &font::DIGITS[*d] };
        draw_glyph(glyph, x0 + (i as i32) * GLYPH_ADV, y0, 0xFFE8E8EC);
    }
}

// --- mouse cursor (own arrow, background save/restore) ---
const CUR_W: i32 = 12;
const CUR_H: i32 = 19;

// classic arrow, bit 11 = left column
const CURSOR_SHAPE: [u16; 19] = [
    0x800, 0xC00, 0xE00, 0xF00, 0xF80, 0xFC0, 0xFE0, 0xFF0, 0xFF8, 0xFFC,
    0xFFE, 0xFFC, 0xFE0, 0xF80, 0xEC0, 0x860, 0x030, 0x018, 0x00C,
];

static mut CUR_X: i32 = 0;
static mut CUR_Y: i32 = 0;
static mut CUR_BG: [u32; 228] = [0; 228];

fn save_cursor_bg() {
    unsafe {
        let (x, y) = (CUR_X, CUR_Y);
        for ry in 0..CUR_H {
            for rx in 0..CUR_W {
                let fx = clamp(x + rx, 0, W - 1);
                let fy = clamp(y + ry, 0, H - 1);
                CUR_BG[(ry * CUR_W + rx) as usize] =
                    *FB.offset((fy * PPSL + fx) as isize);
            }
        }
    }
}

fn restore_cursor_bg() {
    unsafe {
        let (x, y) = (CUR_X, CUR_Y);
        for ry in 0..CUR_H {
            for rx in 0..CUR_W {
                let c = CUR_BG[(ry * CUR_W + rx) as usize];
                px(x + rx, y + ry, c);
            }
        }
    }
}

fn draw_cursor() {
    unsafe {
        let (x, y) = (CUR_X, CUR_Y);
        for (ry, row) in CURSOR_SHAPE.iter().enumerate() {
            for rx in 0..CUR_W {
                if row & (0x800 >> rx) != 0 {
                    px(x + rx, y + ry as i32, 0xFF14141A); // dark arrow
                }
            }
        }
    }
}

pub fn init_cursor() {
    unsafe {
        CUR_X = W / 2 - 6;
        CUR_Y = H / 2 - 10;
    }
    save_cursor_bg();
    draw_cursor();
}

/// called from the IRQ12 dispatcher
pub fn move_cursor(dx: i32, dy: i32, _buttons: u8, click: bool) {
    restore_cursor_bg();
    unsafe {
        CUR_X = clamp(CUR_X + dx, 0, W - CUR_W);
        CUR_Y = clamp(CUR_Y + dy, 0, H - CUR_H);
    }
    if click {
        // click feedback: blend a soft bright rounded square
        let cx = unsafe { CUR_X + 6 };
        let cy = unsafe { CUR_Y + 10 };
        round_rect_blend(cx - 22, cy - 22, 44, 44, 14, 0xFFFFFF, 60);
    }
    save_cursor_bg();
    draw_cursor();
}

pub fn draw_dock() {
    let (w, h) = (width(), height());
    crate::serial::log("dock start\n");
    let n = 6;
    let is = 44;
    let gap = 10;
    let pad = 12;
    let dw = n * is + (n - 1) * gap + 2 * pad;
    let dx0 = (w - dw) / 2;
    let dy0 = h - (is + 2 * pad) - 10;
    let dh = is + 2 * pad;

    round_rect_blend(dx0, dy0, dw, dh, 18, 0x14141A, 125);
    round_rect_stroke(dx0, dy0, dw, dh, 18, 2, 0x3A3A46);
    crate::serial::log("dock bg ok\n");

    let tops: [u32; 6] = [0xFF4C9AFF, 0xFF34D399, 0xFFFBBF24, 0xFFA78BFA, 0xFFF87171, 0xFF94A3B8];
    let bots: [u32; 6] = [0xFF1D4ED8, 0xFF059669, 0xFFD97706, 0xFF7C3AED, 0xFFDC2626, 0xFF475569];
    for i in 0..n as usize {
        let ix = dx0 + pad + i as i32 * (is + gap);
        let iy = dy0 + pad;
        crate::serial::log("icon ");
        crate::serial::log_num(i);
        crate::serial::log("\n");
        // icon body: vertical gradient
        for y in 0..is {
            let tt = y * 255 / (is - 1);
            let c = mix(tops[i], bots[i], tt);
            let d = rr_row(is, 12, y);
            for x in ix + 12 - d..=ix + is - 1 - 12 + d {
                px_blend(x, iy + y, c, 255);
            }
        }
        let cx = ix + is / 2;
        let cy = iy + is / 2;
        let wh: u32 = 0xFFFFFFFF;
        match i {
            0 => {
                ring_circle(cx, cy, 9, 3, wh);
                fill_rect(cx - 2, cy + 13, 4, 3, wh);
            }
            1 => {
                fill_rect(cx - 12, cy - 10, 24, 7, wh);
                fill_rect(cx - 12, cy + 3, 24, 7, wh);
            }
            2 => {
                draw_line_t(cx - 11, cy + 9, cx, cy - 10, 3, wh);
                draw_line_t(cx, cy - 10, cx + 11, cy + 9, 3, wh);
                draw_line_t(cx - 11, cy + 9, cx + 11, cy + 9, 3, wh);
            }
            3 => {
                draw_line_t(cx - 10, cy - 10, cx + 10, cy - 10, 3, wh);
                draw_line_t(cx + 10, cy - 10, cx + 10, cy + 10, 3, wh);
                draw_line_t(cx + 10, cy + 10, cx - 10, cy + 10, 3, wh);
                draw_line_t(cx - 10, cy + 10, cx - 10, cy - 10, 3, wh);
            }
            4 => {
                ring_circle(cx, cy, 8, 3, wh);
                draw_line_t(cx, cy - 13, cx, cy - 9, 3, wh);
                draw_line_t(cx, cy + 9, cx, cy + 13, 3, wh);
                draw_line_t(cx - 13, cy, cx - 9, cy, 3, wh);
                draw_line_t(cx + 9, cy, cx + 13, cy, 3, wh);
            }
            _ => {
                draw_line_t(cx - 9, cy + 9, cx + 9, cy - 9, 3, wh);
                fill_rect(cx - 12, cy + 6, 5, 5, wh);
            }
        }
    }
    // launch indicator under icon 1
    blend_rect(dx0 + pad + is / 2 - 2, dy0 + is + pad + 4, 4, 4, 0xFFFFFF, 210);
    crate::serial::log("dock marker ok\n");
    let _ = circle_fill;
}
