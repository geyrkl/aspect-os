// ASPECT OS - own GUI library in Rust: draw targets (framebuffer / background
// buffer), clipping, themes, bitmap text, desktop rendering, cursor.
// Everything is our own code - no external crates, no foreign sources.
#![allow(unknown_lints, static_mut_refs, dead_code)]
use crate::bootinfo::BootInfo;
use crate::font;

// ---- framebuffer (screen) ----
static mut FB: *mut u32 = core::ptr::null_mut();
static mut W: i32 = 0;
static mut H: i32 = 0;
static mut PPSL: i32 = 0;

// ---- background buffer: the pre-rendered desktop (gradient + menubar +
// wordmark + dock). The compositor blits this every frame, then draws the
// windows on top. Fixed 2048x2048 covers every screen we target.
pub const BG_W: i32 = 2048;
pub const BG_H: i32 = 2048;
pub static mut BG: [u32; (BG_W * BG_H) as usize] = [0; (BG_W * BG_H) as usize];
static mut BG_OK: bool = false;

// ---- current draw target (framebuffer, background, or app canvas) ----
static mut T: *mut u32 = core::ptr::null_mut();
static mut TW: i32 = 0;
static mut TH: i32 = 0;
static mut TPPSL: i32 = 0;

// ---- clip rect (inclusive, absolute coordinates in the current target) ----
static mut CL_X0: i32 = 0;
static mut CL_Y0: i32 = 0;
static mut CL_X1: i32 = 0;
static mut CL_Y1: i32 = 0;

pub fn fb_init(bi: &BootInfo) {
    unsafe {
        FB = bi.fb_base as *mut u32;
        W = bi.fb_width as i32;
        H = bi.fb_height as i32;
        PPSL = bi.fb_pitch_px as i32;
        set_target_fb();
    }
}

/// draw into the framebuffer
pub fn set_target_fb() {
    unsafe {
        T = FB;
        TW = W;
        TH = H;
        TPPSL = PPSL;
    }
    clear_clip();
}

/// draw into an arbitrary pixel buffer
pub fn set_target(p: *mut u32, w: i32, h: i32, ppsl: i32) {
    unsafe {
        T = p;
        TW = w;
        TH = h;
        TPPSL = ppsl;
    }
    clear_clip();
}

/// draw into the background buffer (screen-sized window into the 2048x2048 BG)
pub fn set_target_bg() {
    unsafe {
        let w = if W < BG_W { W } else { BG_W };
        let h = if H < BG_H { H } else { BG_H };
        T = BG.as_mut_ptr();
        TW = w;
        TH = h;
        TPPSL = BG_W;
    }
    clear_clip();
}

// target dimensions (desktop layout uses these)
pub fn tw() -> i32 {
    unsafe { TW }
}
pub fn th() -> i32 {
    unsafe { TH }
}

// screen dimensions
pub fn width() -> i32 {
    unsafe { W }
}
pub fn height() -> i32 {
    unsafe { H }
}

// ---- clipping ----
pub fn clear_clip() {
    unsafe {
        CL_X0 = 0;
        CL_Y0 = 0;
        CL_X1 = TW - 1;
        CL_Y1 = TH - 1;
    }
}

pub fn set_clip(x: i32, y: i32, w: i32, h: i32) {
    unsafe {
        let xa = if x < 0 { 0 } else { x };
        let ya = if y < 0 { 0 } else { y };
        let xb = (x + w - 1).min(TW - 1);
        let yb = (y + h - 1).min(TH - 1);
        if xa > xb || ya > yb {
            // empty clip: reject everything
            CL_X0 = 0;
            CL_Y0 = 0;
            CL_X1 = -1;
            CL_Y1 = -1;
        } else {
            CL_X0 = xa;
            CL_Y0 = ya;
            CL_X1 = xb;
            CL_Y1 = yb;
        }
    }
}

// colors are 0xAARRGGBB
#[inline]
pub fn mix(d: u32, s: u32, a: i32) -> u32 {
    let dr = ((d >> 16) & 255) as i32;
    let dg = ((d >> 8) & 255) as i32;
    let db = (d & 255) as i32;
    let sr = ((s >> 16) & 255) as i32;
    let sg = ((s >> 8) & 255) as i32;
    let sb = (s & 255) as i32;
    let r = dr + (((sr - dr) * a) >> 8);
    let g = dg + (((sg - dg) * a) >> 8);
    let b = db + (((sb - db) * a) >> 8);
    0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

#[inline]
fn px(x: i32, y: i32, c: u32) {
    unsafe {
        if x < CL_X0 || x > CL_X1 || y < CL_Y0 || y > CL_Y1 {
            return;
        }
        *T.offset((y * TPPSL + x) as isize) = c;
    }
}

/// plot a single pixel into the current target (clipped)
pub fn pset(x: i32, y: i32, c: u32) {
    px(x, y, c)
}

pub fn px_blend(x: i32, y: i32, c: u32, a: i32) {
    unsafe {
        if x < CL_X0 || x > CL_X1 || y < CL_Y0 || y > CL_Y1 {
            return;
        }
        let p = T.offset((y * TPPSL + x) as isize);
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

pub fn fill_rect(x0: i32, y0: i32, w: i32, h: i32, c: u32) {
    unsafe {
        let xa = clamp(x0, CL_X0, CL_X1 + 1);
        let xb = clamp(x0 + w - 1, CL_X0, CL_X1 + 1);
        let ya = clamp(y0, CL_Y0, CL_Y1 + 1);
        let yb = clamp(y0 + h - 1, CL_Y0, CL_Y1 + 1);
        let mut y = ya;
        while y <= yb {
            let row = T.offset((y * TPPSL) as isize);
            let mut x = xa;
            while x <= xb {
                *row.offset(x as isize) = c;
                x += 1;
            }
            y += 1;
        }
    }
}

pub fn blend_rect(x0: i32, y0: i32, w: i32, h: i32, c: u32, a: i32) {
    unsafe {
        let xa = clamp(x0, CL_X0, CL_X1 + 1);
        let xb = clamp(x0 + w - 1, CL_X0, CL_X1 + 1);
        let ya = clamp(y0, CL_Y0, CL_Y1 + 1);
        let yb = clamp(y0 + h - 1, CL_Y0, CL_Y1 + 1);
        let mut y = ya;
        while y <= yb {
            let row = T.offset((y * TPPSL) as isize);
            let mut x = xa;
            while x <= xb {
                let p = row.offset(x as isize);
                *p = mix(*p, c, a);
                x += 1;
            }
            y += 1;
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
pub fn rr_row(h: i32, r: i32, y: i32) -> i32 {
    if y < r {
        isqrt((r * r - (r - 1 - y) * (r - 1 - y)) as u64) as i32
    } else if y >= h - r {
        isqrt((r * r - (y - (h - r)) * (y - (h - r))) as u64) as i32
    } else {
        r
    }
}

pub fn round_rect_fill(x0: i32, y0: i32, w: i32, h: i32, mut r: i32, c: u32) {
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

pub fn round_rect_blend(x0: i32, y0: i32, w: i32, h: i32, mut r: i32, c: u32, a: i32) {
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
pub fn round_rect_stroke(x0: i32, y0: i32, w: i32, h: i32, mut r: i32, t: i32, c: u32) {
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
pub fn circle_fill(cx: i32, cy: i32, r: i32, c: u32) {
    for y in -r..=r {
        for x in -r..=r {
            if x * x + y * y <= r * r {
                px(cx + x, cy + y, c);
            }
        }
    }
}

// circle ring
pub fn ring_circle(cx: i32, cy: i32, r: i32, t: i32, c: u32) {
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

// ---- themes (desktop gradient palettes) ----
pub const THEME_NAMES: [&str; 4] = ["AURORA", "SUNSET", "OCEAN", "FOREST"];

// corner colors: [top-left, top-right, bottom-left, bottom-right]
const THEME_COLS: [[u32; 4]; 4] = [
    [0xFF292D63, 0xFF6B317F, 0xFF592D6D, 0xFF942E7D], // AURORA (classic)
    [0xFF6E2C3C, 0xFFD97A3C, 0xFF943A63, 0xFFE8B05A], // SUNSET
    [0xFF173F66, 0xFF2E86C1, 0xFF0E5E6F, 0xFF3EC1C9], // OCEAN
    [0xFF1E4620, 0xFF4A7A2E, 0xFF14402E, 0xFF7AA53C], // FOREST
];

static mut THEME: usize = 0;

pub fn theme_count() -> usize {
    THEME_NAMES.len()
}

pub fn theme_name(i: usize) -> &'static str {
    THEME_NAMES[i % THEME_NAMES.len()]
}

pub fn set_theme(i: usize) {
    unsafe {
        THEME = i % THEME_NAMES.len();
    }
}

pub fn theme() -> usize {
    unsafe { THEME }
}

/// gradient color at a single pixel (same formula as draw_gradient)
pub fn gradient_color(x: i32, y: i32) -> u32 {
    unsafe {
        let w = TW;
        let h = TH;
        let u = ((x as i64) * 256 / ((w - 1).max(1) as i64)) as i32;
        let v = ((y as i64) * 256 / ((h - 1).max(1) as i64)) as i32;
        let cols = THEME_COLS[THEME];
        let (c00, c10, c01, c11) = (cols[0], cols[1], cols[2], cols[3]);
        let w00 = (256 - u) * (256 - v);
        let w10 = u * (256 - v);
        let w01 = (256 - u) * v;
        let w11 = u * v;
        let r = (w00 * ((c00 >> 16) & 255) as i32
            + w10 * ((c10 >> 16) & 255) as i32
            + w01 * ((c01 >> 16) & 255) as i32
            + w11 * ((c11 >> 16) & 255) as i32)
            >> 16;
        let g = (w00 * ((c00 >> 8) & 255) as i32
            + w10 * ((c10 >> 8) & 255) as i32
            + w01 * ((c01 >> 8) & 255) as i32
            + w11 * ((c11 >> 8) & 255) as i32)
            >> 16;
        let b = (w00 * (c00 & 255) as i32
            + w10 * (c10 & 255) as i32
            + w01 * (c01 & 255) as i32
            + w11 * (c11 & 255) as i32)
            >> 16;
        0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | b as u32
    }
}

pub fn draw_gradient() {
    unsafe {
        let (w, h) = (TW, TH);
        let cols = THEME_COLS[THEME];
        let (c00, c10, c01, c11) = (cols[0], cols[1], cols[2], cols[3]);
        for y in 0..h {
            let v = ((y as i64) * 256 / (h - 1).max(1) as i64) as i32;
            let row = T.offset((y * TPPSL) as isize);
            for x in 0..w {
                let u = ((x as i64) * 256 / (w - 1).max(1) as i64) as i32;
                let w00 = (256 - u) * (256 - v);
                let w10 = u * (256 - v);
                let w01 = (256 - u) * v;
                let w11 = u * v;
                let r = (w00 * ((c00 >> 16) & 255) as i32
                    + w10 * ((c10 >> 16) & 255) as i32
                    + w01 * ((c01 >> 16) & 255) as i32
                    + w11 * ((c11 >> 16) & 255) as i32)
                    >> 16;
                let g = (w00 * ((c00 >> 8) & 255) as i32
                    + w10 * ((c10 >> 8) & 255) as i32
                    + w01 * ((c01 >> 8) & 255) as i32
                    + w11 * ((c11 >> 8) & 255) as i32)
                    >> 16;
                let b = (w00 * (c00 & 255) as i32
                    + w10 * (c10 & 255) as i32
                    + w01 * (c01 & 255) as i32
                    + w11 * (c11 & 255) as i32)
                    >> 16;
                *row.offset(x as isize) =
                    0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | b as u32;
            }
        }
    }
}

/// the stylized "A" monogram in the menubar (drawn again over menu highlights)
pub fn draw_monogram() {
    draw_line_t(14, 21, 18, 7, 2, 0xFFB9A8F5);
    draw_line_t(18, 7, 22, 21, 2, 0xFFF5A8E8);
    draw_line_t(16, 15, 20, 15, 2, 0xFFFFFFFF);
}

pub fn draw_menubar() {
    let w = tw();
    blend_rect(0, 0, w, 28, 0x0A0A0C, 150);
    blend_rect(0, 27, w, 1, 0xFFFFFF, 30);

    // monogram on the left: stylized "A" (the app menu)
    draw_monogram();

    // right side: wifi + battery
    let wx = w - 96;
    wifi_arc(wx, 24, 9, 0xFFE8E8EC);
    wifi_arc(wx, 24, 5, 0xFFE8E8EC);
    fill_rect(wx - 1, 20, 3, 3, 0xFFE8E8EC);
    let bx = w - 66;
    round_rect_stroke(bx, 9, 26, 13, 3, 2, 0xFFC8C8CC);
    fill_rect(bx + 3, 12, 15, 7, 0xFFE8E8EC);
    fill_rect(bx + 27, 12, 2, 7, 0xFFC8C8CC);
}

// --- clock in the menubar (HH:MM:SS, bitmap digits) ---
const CLOCK_SCALE: i32 = 2;
const GLYPH_ADV: i32 = (font::GLYPH_W as i32 + 2) * CLOCK_SCALE; // advance per glyph
const CLOCK_TW: i32 = 8 * GLYPH_ADV - 4 * CLOCK_SCALE;
const CLOCK_X0: i32 = 96 + 8 + CLOCK_TW; // measured from the right edge

fn clock_glyph(g: &[u8; 7], x0: i32, y0: i32, c: u32) {
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

/// draw the menubar clock onto the framebuffer (repaints its area from BG)
pub fn draw_clock(secs: u64) {
    unsafe {
        clear_clip();
        if !BG_OK {
            return;
        }
        let x0 = W - CLOCK_X0;
        let y0 = 7;

        // repaint the clock area from the background buffer
        for y in (y0 - 1)..(y0 + 16) {
            if y < 0 || y >= H || y >= BG_H {
                continue;
            }
            for x in (x0 - 2)..(x0 + CLOCK_TW + 2) {
                if x < 0 || x >= W || x >= BG_W {
                    continue;
                }
                let c = BG[y as usize * BG_W as usize + x as usize];
                *FB.offset((y * PPSL + x) as isize) = c;
            }
        }

        // H H : M M : S S
        let h = ((secs / 3600) % 24) as usize;
        let m = ((secs / 60) % 60) as usize;
        let s = (secs % 60) as usize;
        let seq: [(usize, bool); 8] = [
            (h / 10, false), (h % 10, false), (0, true),
            (m / 10, false), (m % 10, false), (0, true),
            (s / 10, false), (s % 10, false),
        ];
        for (i, (d, colon)) in seq.iter().enumerate() {
            let glyph: &[u8; 7] = if *colon { &font::COLON } else { &font::DIGITS[*d] };
            clock_glyph(glyph, x0 + (i as i32) * GLYPH_ADV, y0, 0xFFE8E8EC);
        }
    }
}

// --- ASCII bitmap text (5x7 font, scaled) ---
pub fn text_w(s: &str, sc: i32) -> i32 {
    text_w_b(s.as_bytes(), sc)
}

pub fn text_w_b(b: &[u8], sc: i32) -> i32 {
    if b.is_empty() {
        0
    } else {
        (b.len() as i32 * 6 - 1) * sc
    }
}

fn glyph5(ch: u8, x0: i32, y0: i32, sc: i32, c: u32) {
    let idx = if ch < 32 || ch > 126 { 0 } else { (ch - 32) as usize };
    for row in 0..font::A_H {
        let bits = font::ASCII5[idx][row];
        for col in 0..font::A_W {
            if bits & (0x10 >> col) != 0 {
                for oy in 0..sc {
                    for ox in 0..sc {
                        px(x0 + (col as i32) * sc + ox, y0 + (row as i32) * sc + oy, c);
                    }
                }
            }
        }
    }
}

/// draw ASCII text (lowercase renders as uppercase glyphs)
pub fn draw_text(x: i32, y: i32, s: &str, sc: i32, c: u32) {
    draw_text_b(x, y, s.as_bytes(), sc, c);
}

/// draw ASCII text from raw bytes (for app text buffers)
pub fn draw_text_b(x: i32, y: i32, b: &[u8], sc: i32, c: u32) {
    let mut gx = x;
    for &ch in b {
        if ch != b' ' {
            glyph5(ch, gx, y, sc, c);
        }
        gx += 6 * sc;
    }
}

// --- dock (dynamic: cursor magnification, launch bounce, running dots) ---
pub const DOCK_N: usize = 8;
const DOCK_IS: i32 = 44; // base icon size
const DOCK_GAP: i32 = 10;
const DOCK_PAD: i32 = 12;

// layout of the last drawn dock (hit tests between frames read this)
static mut D_X: [i32; DOCK_N] = [0; DOCK_N];
static mut D_Y: [i32; DOCK_N] = [0; DOCK_N];
static mut D_S: [i32; DOCK_N] = [0; DOCK_N];
static mut D_PANEL: (i32, i32, i32, i32) = (0, 0, 0, 0);

/// compute and store the icon layout for the current magnification
/// (mag[i]: 0..=256, 256 = fully magnified, +16 px)
fn dock_layout(mag: &[i32; DOCK_N]) {
    unsafe {
        let mut w = DOCK_PAD * 2;
        for i in 0..DOCK_N {
            D_S[i] = DOCK_IS + (mag[i] * 16 >> 8);
            w += D_S[i];
        }
        w += (DOCK_N as i32 - 1) * DOCK_GAP;
        let x0 = (tw() - w) / 2;
        let yb = th() - 10 - DOCK_PAD; // bottom edge of the icon row
        D_PANEL = (x0, yb - DOCK_IS - DOCK_PAD, w, DOCK_IS + 2 * DOCK_PAD);
        let mut x = x0 + DOCK_PAD;
        for i in 0..DOCK_N {
            D_X[i] = x;
            D_Y[i] = yb - D_S[i]; // icons stay anchored at the bottom
            x += D_S[i] + DOCK_GAP;
        }
    }
}

/// which dock icon sits under (cx, cy)? -1 = none
pub fn dock_hit(cx: i32, cy: i32) -> i32 {
    unsafe {
        let (px, py, pw, ph) = D_PANEL;
        if cx < px || cx >= px + pw || cy < py - 24 || cy >= py + ph + 8 {
            return -1;
        }
        for i in 0..DOCK_N {
            if cx >= D_X[i] - DOCK_GAP / 2 && cx < D_X[i] + D_S[i] + DOCK_GAP / 2 {
                return i as i32;
            }
        }
    }
    -1
}

/// draw the dock for one frame: panel, icons magnified around the cursor,
/// launch bounce lift, running-app indicator dots
pub fn draw_dock(mag: &[i32; DOCK_N], bounce: i32, lift: i32, running: &[bool; DOCK_N]) {
    dock_layout(mag);
    let (dpx, dpy, dpw, dph) = unsafe { D_PANEL };
    round_rect_blend(dpx, dpy, dpw, dph, 18, 0x14141A, 125);
    round_rect_stroke(dpx, dpy, dpw, dph, 18, 2, 0x3A3A46);

    let tops: [u32; DOCK_N] = [
        0xFF4C9AFF, 0xFF34D399, 0xFFFBBF24, 0xFFA78BFA,
        0xFFF97316, 0xFFF472B6, 0xFFF87171, 0xFF94A3B8,
    ];
    let bots: [u32; DOCK_N] = [
        0xFF1D4ED8, 0xFF059669, 0xFFD97706, 0xFF7C3AED,
        0xFFC2410C, 0xFFDB2777, 0xFFDC2626, 0xFF475569,
    ];
    let dot_y = dpy + dph - 4;
    for i in 0..DOCK_N {
        let (ix, iy0, is) = unsafe { (D_X[i], D_Y[i], D_S[i]) };
        let mut iy = iy0;
        if i as i32 == bounce {
            iy -= lift; // launch bounce hops above the panel
        }
        // icon body: vertical gradient
        for y in 0..is {
            let tt = y * 255 / (is - 1);
            let c = mix(tops[i], bots[i], tt);
            let d = rr_row(is, 12, y);
            for x in ix + 12 - d..=ix + is - 1 - 12 + d {
                px(x, iy + y, c);
            }
        }
        let cx = ix + is / 2;
        let cy = iy + is / 2;
        let wh: u32 = 0xFFFFFFFF;
        match i {
            0 => {
                // browser: globe
                ring_circle(cx, cy, 9, 3, wh);
                fill_rect(cx - 2, cy + 13, 4, 3, wh);
            }
            1 => {
                // files: two bars
                fill_rect(cx - 12, cy - 10, 24, 7, wh);
                fill_rect(cx - 12, cy + 3, 24, 7, wh);
            }
            2 => {
                // paint: brush triangle
                draw_line_t(cx - 11, cy + 9, cx, cy - 10, 3, wh);
                draw_line_t(cx, cy - 10, cx + 11, cy + 9, 3, wh);
                draw_line_t(cx - 11, cy + 9, cx + 11, cy + 9, 3, wh);
            }
            3 => {
                // terminal: frame
                draw_line_t(cx - 10, cy - 10, cx + 10, cy - 10, 3, wh);
                draw_line_t(cx + 10, cy - 10, cx + 10, cy + 10, 3, wh);
                draw_line_t(cx + 10, cy + 10, cx - 10, cy + 10, 3, wh);
                draw_line_t(cx - 10, cy + 10, cx - 10, cy - 10, 3, wh);
            }
            4 => {
                // notes: page with lines
                fill_rect(cx - 9, cy - 12, 18, 24, wh);
                fill_rect(cx - 6, cy - 7, 12, 2, bots[i]);
                fill_rect(cx - 6, cy - 1, 12, 2, bots[i]);
                fill_rect(cx - 6, cy + 5, 8, 2, bots[i]);
            }
            5 => {
                // music: eighth note
                circle_fill(cx - 5, cy + 7, 4, wh);
                draw_line_t(cx - 1, cy + 7, cx - 1, cy - 10, 2, wh);
                draw_line_t(cx - 1, cy - 10, cx + 8, cy - 5, 2, wh);
            }
            6 => {
                // settings: gear-ish
                ring_circle(cx, cy, 8, 3, wh);
                draw_line_t(cx, cy - 13, cx, cy - 9, 3, wh);
                draw_line_t(cx, cy + 9, cx, cy + 13, 3, wh);
                draw_line_t(cx - 13, cy, cx - 9, cy, 3, wh);
                draw_line_t(cx + 9, cy, cx + 13, cy, 3, wh);
            }
            _ => {
                // calculator: slash + pad
                draw_line_t(cx - 9, cy + 9, cx + 9, cy - 9, 3, wh);
                fill_rect(cx - 12, cy + 6, 5, 5, wh);
            }
        }
        if running[i] {
            circle_fill(ix + is / 2, dot_y, 2, 0xFFF0F0F4);
        }
    }
}

// --- desktop background: render once, blit every frame ---
// (the dock is NOT baked in: it is drawn per frame so it can magnify,
// bounce and show running indicators)
fn bg_render() {
    set_target_bg();
    draw_gradient();
    draw_menubar();
    crate::logo::draw_wordmark(tw() / 2, th() * 40 / 100, 5);
    set_target_fb();
}

pub fn bg_init() {
    bg_render();
    unsafe {
        BG_OK = true;
    }
}

/// re-render the background (after a theme change)
pub fn bg_regen() {
    unsafe {
        if BG_OK {
            bg_render();
        }
    }
}

/// copy the pre-rendered desktop into the framebuffer (u64 bulk per row)
pub fn blit_bg() {
    unsafe {
        if !BG_OK {
            return;
        }
        let w = if W < BG_W { W } else { BG_W };
        let h = if H < BG_H { H } else { BG_H };
        if w <= 0 || h <= 0 {
            return;
        }
        let wu = w as usize;
        for y in 0..h {
            let src = BG.as_ptr().add(y as usize * BG_W as usize);
            let dst = FB.add(y as usize * PPSL as usize);
            if (src as usize) & 7 == 0 && (dst as usize) & 7 == 0 {
                let s64 = src as *const u64;
                let d64 = dst as *mut u64;
                for i in 0..wu / 2 {
                    *d64.add(i) = *s64.add(i);
                }
                if wu & 1 != 0 {
                    *dst.add(wu - 1) = *src.add(wu - 1);
                }
            } else {
                for x in 0..wu {
                    *dst.add(x) = *src.add(x);
                }
            }
        }
    }
}

// --- mouse cursor (own arrow; the compositor draws it last, no save/restore) ---
const CUR_W: i32 = 12;
const CUR_H: i32 = 19;

// classic arrow, bit 11 = left column
const CURSOR_SHAPE: [u16; 19] = [
    0x800, 0xC00, 0xE00, 0xF00, 0xF80, 0xFC0, 0xFE0, 0xFF0, 0xFF8, 0xFFC,
    0xFFE, 0xFFC, 0xFE0, 0xF80, 0xEC0, 0x860, 0x030, 0x018, 0x00C,
];

static mut CUR_X: i32 = 0;
static mut CUR_Y: i32 = 0;

pub fn cursor_pos() -> (i32, i32) {
    unsafe { (CUR_X, CUR_Y) }
}

pub fn set_cursor(x: i32, y: i32) {
    unsafe {
        CUR_X = clamp(x, 0, W - CUR_W);
        CUR_Y = clamp(y, 0, H - CUR_H);
    }
}

/// just place the cursor; drawing happens in the compositor redraw
pub fn init_cursor() {
    unsafe {
        CUR_X = W / 2 - 6;
        CUR_Y = H / 2 - 10;
    }
}

/// draw the arrow at the current position into the current target (framebuffer)
pub fn draw_cursor() {
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
