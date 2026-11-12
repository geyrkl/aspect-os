// ASPECT OS - eigene GUI-Bibliothek in Rust: zeichnet direkt auf den Framebuffer
// Gradient, Menueleiste, Dock - alles eigener Code.
use crate::bootinfo::BootInfo;

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

// Farben sind 0xAARRGGBB
#[inline]
fn mix(d: u32, s: u32, a: i32) -> u32 {
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

// Scanline-Extents eines Rundrechtecks in Zeile y (0-basiert innerhalb)
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

// Abgerundetes Rechteck als Rand (Signed-Distance, nur Ganzzahlen)
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

// Dicker Linien-Strich (Bresenham + quadratischer Pinsel)
pub fn draw_line_t(x0: i32, y0: i32, x1: i32, y1: i32, t: i32, c: u32) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let adx = dx.abs();
    let ady = dy.abs();
    let sx = if dx > 0 { 1 } else { -1 };
    let sy = if dy > 0 { 1 } else { -1 };
    let mut err = adx - ady; // klassischer Bresenham-Startwert
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

// Voll gefuellter Kreis
fn circle_fill(cx: i32, cy: i32, r: i32, c: u32) {
    for y in -r..=r {
        for x in -r..=r {
            if x * x + y * y <= r * r {
                px(cx + x, cy + y, c);
            }
        }
    }
}

// Kreisring
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

// Wifi-Bogen (Kegel nach oben)
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

// --- Desktop-Farben (LiquidGlass-Referenz-Gradient) ---
const C00: u32 = 0xFF29_2D63; // oben links
const C10: u32 = 0xFF6B_317F; // oben rechts
const C01: u32 = 0xFF59_2D6D; // unten links
const C11: u32 = 0xFF94_2E7D; // unten rechts

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

    // Monogramm links: stilisiertes "A"
    draw_line_t(14, 21, 18, 7, 2, 0xFFB9A8F5);
    draw_line_t(18, 7, 22, 21, 2, 0xFFF5A8E8);
    draw_line_t(16, 15, 20, 15, 2, 0xFFFFFFFF);

    // rechts: Wifi + Batterie
    let wx = w - 96;
    wifi_arc(wx, 24, 9, 0xFFE8E8EC);
    wifi_arc(wx, 24, 5, 0xFFE8E8EC);
    fill_rect(wx - 1, 20, 3, 3, 0xFFE8E8EC);
    let bx = w - 66;
    round_rect_stroke(bx, 9, 26, 13, 3, 2, 0xFFC8C8CC);
    fill_rect(bx + 3, 12, 15, 7, 0xFFE8E8EC);
    fill_rect(bx + 27, 12, 2, 7, 0xFFC8C8CC);
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
        // Icon-Koerper: vertikaler Gradient
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
    // Startpunkt-Marker unter Icon 1
    blend_rect(dx0 + pad + is / 2 - 2, dy0 + is + pad + 4, 4, 4, 0xFFFFFF, 210);
    crate::serial::log("dock marker ok\n");
    let _ = circle_fill;
}
