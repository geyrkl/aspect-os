// ASPECT OS - window manager + compositor: window list (z-order by index),
// mouse/key ring buffers (IRQ producer, main-loop consumer), drag, focus,
// traffic lights, dock launching, and the full-screen redraw.
#![allow(unknown_lints, static_mut_refs, dead_code)]
use crate::apps::{self, AppId};
use crate::gui;
use core::sync::atomic::{AtomicUsize, Ordering};

pub const MAXW: usize = 8;
pub const TITLE_H: i32 = 30;
const MIN_Y: i32 = 36; // windows never cover the menubar
const NO_DRAG: usize = usize::MAX;

#[derive(Clone, Copy)]
pub struct Win {
    pub app: AppId,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

// z-order: index 0 = back, index N-1 = front (focused)
static mut WINS: [Option<Win>; MAXW] = [None; MAXW];
static mut N: usize = 0;
static mut DIRTY: bool = false;

// ---- input queues (IRQ producer, main-loop consumer) ----
const QN: usize = 128;

#[derive(Clone, Copy)]
pub struct MEv {
    pub dx: i32,
    pub dy: i32,
    pub btn: u8,
    pub click: bool,
}

static mut MQ: [MEv; QN] = [MEv { dx: 0, dy: 0, btn: 0, click: false }; QN];
static MQ_HEAD: AtomicUsize = AtomicUsize::new(0);
static MQ_TAIL: AtomicUsize = AtomicUsize::new(0);

static mut KQ: [u32; QN] = [0; QN];
static KQ_HEAD: AtomicUsize = AtomicUsize::new(0);
static KQ_TAIL: AtomicUsize = AtomicUsize::new(0);

/// called from the IRQ12 dispatcher
pub fn push_mouse(dx: i32, dy: i32, btn: u8, click: bool) {
    let t = MQ_TAIL.load(Ordering::Relaxed);
    let h = MQ_HEAD.load(Ordering::Acquire);
    if t.wrapping_sub(h) >= QN {
        return; // queue full: drop
    }
    unsafe {
        MQ[t % QN] = MEv { dx, dy, btn, click };
    }
    MQ_TAIL.store(t.wrapping_add(1), Ordering::Release);
}

fn pop_mouse() -> Option<MEv> {
    let h = MQ_HEAD.load(Ordering::Relaxed);
    let t = MQ_TAIL.load(Ordering::Acquire);
    if h == t {
        return None;
    }
    let ev = unsafe { MQ[h % QN] };
    MQ_HEAD.store(h + 1, Ordering::Release);
    Some(ev)
}

/// called from the IRQ1 dispatcher; code: 1..=255 ASCII, 256 Enter,
/// 257 Backspace, 258 Escape
pub fn push_key(code: u32) {
    let t = KQ_TAIL.load(Ordering::Relaxed);
    let h = KQ_HEAD.load(Ordering::Acquire);
    if t.wrapping_sub(h) >= QN {
        return;
    }
    unsafe {
        KQ[t % QN] = code;
    }
    KQ_TAIL.store(t.wrapping_add(1), Ordering::Release);
}

fn pop_key() -> Option<u32> {
    let h = KQ_HEAD.load(Ordering::Relaxed);
    let t = KQ_TAIL.load(Ordering::Acquire);
    if h == t {
        return None;
    }
    let code = unsafe { KQ[h % QN] };
    KQ_HEAD.store(h + 1, Ordering::Release);
    Some(code)
}

// ---- interactive state (main loop only) ----
static mut HELD: bool = false; // left button held
static mut DRAG: usize = NO_DRAG; // index of the dragged window
static mut GRABX: i32 = 0;
static mut GRABY: i32 = 0;

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

pub fn init() {
    unsafe {
        for i in 0..MAXW {
            WINS[i] = None;
        }
        N = 0;
        DIRTY = false;
        HELD = false;
        DRAG = NO_DRAG;
    }
    MQ_HEAD.store(0, Ordering::Relaxed);
    MQ_TAIL.store(0, Ordering::Relaxed);
    KQ_HEAD.store(0, Ordering::Relaxed);
    KQ_TAIL.store(0, Ordering::Relaxed);
}

pub fn count() -> usize {
    unsafe { N }
}

pub fn take_dirty() -> bool {
    unsafe {
        if DIRTY {
            DIRTY = false;
            true
        } else {
            false
        }
    }
}

// content area of a window (inside the 2px frame, below the title bar,
// stopping above the rounded bottom corners)
fn content_rect(w: Win) -> (i32, i32, i32, i32) {
    (w.x + 2, w.y + TITLE_H, w.w - 4, w.h - TITLE_H - 12)
}

/// move window i to the front; returns its new index
fn raise(i: usize) -> usize {
    unsafe {
        if i + 1 >= N {
            return i;
        }
        let w = WINS[i];
        let mut j = i;
        while j + 1 < N {
            WINS[j] = WINS[j + 1];
            j += 1;
        }
        WINS[N - 1] = w;
        DIRTY = true;
        N - 1
    }
}

fn close(i: usize) {
    unsafe {
        let mut j = i;
        while j + 1 < N {
            WINS[j] = WINS[j + 1];
            j += 1;
        }
        WINS[N - 1] = None;
        N -= 1;
        DRAG = NO_DRAG;
        DIRTY = true;
    }
}

/// open an app window (or raise the existing one)
pub fn open(app: AppId) {
    unsafe {
        for i in 0..N {
            if let Some(w) = WINS[i] {
                if w.app == app {
                    raise(i);
                    return;
                }
            }
        }
        if N >= MAXW {
            close(0); // make room: close the oldest window
        }
        let (dw, dh) = apps::default_size(app);
        let (sw, sh) = (gui::width(), gui::height());
        let w = dw.min(sw - 16);
        let h = dh.min(sh - 80);
        let k = N as i32;
        let x = (96 + 40 * (k % 6)).min(sw - w - 8).max(8);
        let y = (64 + 40 * (k % 6)).max(MIN_Y).min(sh - h - 8);
        WINS[N] = Some(Win { app, x, y, w, h });
        N += 1;
        apps::on_open(app);
        DIRTY = true;
    }
}

fn dock_app(i: usize) -> AppId {
    match i {
        0 => AppId::Browser,
        1 => AppId::Files,
        2 => AppId::Paint,
        3 => AppId::Terminal,
        4 => AppId::Settings,
        _ => AppId::Calc,
    }
}

// left mouse button went down at (cx, cy)
fn press(cx: i32, cy: i32) {
    unsafe {
        // monogram in the menubar opens About
        if cx >= 6 && cx <= 30 && cy >= 4 && cy <= 26 {
            open(AppId::About);
            return;
        }
        // windows front to back
        let mut i = N;
        while i > 0 {
            i -= 1;
            let w = match WINS[i] {
                Some(w) => w,
                None => continue,
            };
            // traffic lights (title bar, left side)
            if cy >= w.y && cy < w.y + TITLE_H && cx >= w.x + 10 && cx < w.x + 58 {
                let hit = |lx: i32| (cx - lx).abs() <= 7 && (cy - (w.y + TITLE_H / 2)).abs() <= 9;
                if hit(w.x + 18) {
                    close(i);
                    return;
                }
                if hit(w.x + 34) || hit(w.x + 50) {
                    return; // yellow/green are decorative for now
                }
            }
            if cx >= w.x && cx < w.x + w.w && cy >= w.y && cy < w.y + w.h {
                let ni = raise(i);
                let w = WINS[ni].unwrap();
                if cy < w.y + TITLE_H {
                    DRAG = ni;
                    GRABX = cx - w.x;
                    GRABY = cy - w.y;
                } else {
                    let (rx, ry, _, _) = content_rect(w);
                    apps::on_click(w.app, cx - rx, cy - ry);
                    DIRTY = true;
                }
                return;
            }
        }
        // dock icons
        for d in 0..gui::DOCK_N {
            let (ix, iy, is) = gui::dock_icon_rect(d);
            if cx >= ix - 6 && cx < ix + is + 6 && cy >= iy - 6 && cy < iy + is + 6 {
                open(dock_app(d));
                return;
            }
        }
    }
}

/// route a held-button move to the focused window's app (e.g. paint strokes)
fn route_move(cx: i32, cy: i32) {
    unsafe {
        if N == 0 || DRAG != NO_DRAG {
            return;
        }
        let w = match WINS[N - 1] {
            Some(w) => w,
            None => return,
        };
        let (rx, ry, rw, rh) = content_rect(w);
        if cx >= rx && cx < rx + rw && cy >= ry && cy < ry + rh {
            apps::on_move(w.app, cx - rx, cy - ry);
            DIRTY = true;
        }
    }
}

/// drain the mouse queue (main loop)
pub fn on_mouse() {
    while let Some(ev) = pop_mouse() {
        // move the cursor
        let (cx, cy) = gui::cursor_pos();
        let cx = clamp(cx + ev.dx, 0, gui::width() - 12);
        let cy = clamp(cy + ev.dy, 0, gui::height() - 19);
        gui::set_cursor(cx, cy);

        let held = ev.btn & 1 != 0;
        if ev.click {
            press(cx, cy);
        } else if held {
            unsafe {
                if DRAG != NO_DRAG {
                    // drag the window by the grab point
                    if let Some(w) = WINS[DRAG] {
                        let mut w = w;
                        let (sw, sh) = (gui::width(), gui::height());
                        w.x = clamp(w.x + ev.dx, 0, sw - w.w);
                        w.y = clamp(w.y + ev.dy, MIN_Y, sh - w.h - 4);
                        WINS[DRAG] = Some(w);
                        DIRTY = true;
                    }
                }
            }
        }
        if !held {
            unsafe {
                DRAG = NO_DRAG;
            }
        }
        unsafe {
            HELD = held;
        }
        if held {
            route_move(cx, cy);
        }
    }
}

/// drain the key queue (main loop); routes to the focused window
pub fn on_key() {
    while let Some(code) = pop_key() {
        unsafe {
            if N == 0 {
                continue;
            }
            let w = match WINS[N - 1] {
                Some(w) => w,
                None => continue,
            };
            apps::on_key(w.app, code);
            DIRTY = true;
        }
    }
}

/// draw one window: shadow, body, title bar, traffic lights, title, content
fn draw_window(focused: bool, w: Win) {
    // drop shadow
    gui::round_rect_blend(w.x - 8, w.y - 2, w.w + 16, w.h + 16, 18, 0x000000,
                          if focused { 110 } else { 70 });
    // body
    gui::round_rect_fill(w.x, w.y, w.w, w.h, 12, 0xFF1B1B21);
    // title strip (kept inside the rounded top corners)
    gui::blend_rect(w.x + 2, w.y + 6, w.w - 4, TITLE_H - 8, 0xFFFFFF,
                    if focused { 16 } else { 7 });
    // traffic lights
    let ly = w.y + TITLE_H / 2;
    gui::circle_fill(w.x + 18, ly, 5, 0xFFFF5F57);
    gui::circle_fill(w.x + 34, ly, 5, 0xFFFEBC2E);
    gui::circle_fill(w.x + 50, ly, 5, 0xFF28C840);
    // title
    let name = apps::title(w.app);
    let tw = gui::text_w(name, 2);
    gui::draw_text(w.x + (w.w - tw) / 2, w.y + 8, name, 2, 0xFFC8C8D0);
    // content (clipped)
    let (cx, cy, cw, ch) = content_rect(w);
    gui::set_clip(cx, cy, cw, ch);
    apps::draw(w.app, cx, cy, cw, ch);
    gui::clear_clip();
}

/// full-screen compositor pass: desktop background, windows back to front, cursor
pub fn redraw() {
    unsafe {
        gui::blit_bg();
        for i in 0..N {
            if let Some(w) = WINS[i] {
                draw_window(i == N - 1, w);
            }
        }
        gui::clear_clip();
        gui::draw_cursor();
    }
}
