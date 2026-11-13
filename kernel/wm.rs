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

// =====================================================================
// macOS-style menubar menus: monogram, app name, FILE, VIEW, WINDOW, HELP
// =====================================================================

const ACCENT: u32 = 0xFF3E5E8E; // menu highlight blue (matches Settings)
const MENU_N: i32 = 6; // 0 monogram, 1 app, 2 FILE, 3 VIEW, 4 WINDOW, 5 HELP
static mut MENU_OPEN: i32 = -1;
static mut MENU_HOVER: i32 = -1; // item row under the cursor

/// the focused app's name for the menubar (macOS shows "Finder" when
/// no window is open)
fn app_menu_name() -> &'static str {
    unsafe {
        if N > 0 {
            if let Some(w) = WINS[N - 1] {
                return apps::title(w.app);
            }
        }
    }
    "FINDER"
}

const MENU_TITLES: [&str; 4] = ["FILE", "VIEW", "WINDOW", "HELP"];

/// hit rect (x0, x1) of menubar menu m
fn menu_rect(m: i32) -> (i32, i32) {
    if m == 0 {
        return (6, 30); // monogram
    }
    let mut x = 38i32;
    let aw = gui::text_w(app_menu_name(), 2);
    if m == 1 {
        return (x, x + aw);
    }
    x += aw + 18;
    for (k, t) in MENU_TITLES.iter().enumerate() {
        let tw = gui::text_w(t, 2);
        if m == k as i32 + 2 {
            return (x, x + tw);
        }
        x += tw + 18;
    }
    (x, x)
}

/// bar title of menu m (empty for the monogram, which is vector strokes)
fn menu_title(m: i32) -> &'static str {
    match m {
        1 => app_menu_name(),
        2 => "FILE",
        3 => "VIEW",
        4 => "WINDOW",
        5 => "HELP",
        _ => "",
    }
}

/// number of rows in menu m (0 = the menu cannot be opened); separator
/// rows count as rows
fn menu_len(m: i32) -> usize {
    match m {
        0 => 3, // ABOUT THIS OS | SETTINGS
        1 => unsafe { if N > 0 { 1 } else { 0 } },
        2 => 6, // four app opens | CLOSE WINDOW
        3 => 4, // themes
        4 => unsafe { N },
        5 => 1, // OPEN MANUAL
        _ => 0,
    }
}

/// label of row r in menu m; None = separator row (or out of range)
fn menu_label(m: i32, r: usize) -> Option<&'static str> {
    if r >= menu_len(m) {
        return None;
    }
    match m {
        0 => match r {
            0 => Some("ABOUT THIS OS"),
            2 => Some("SETTINGS"),
            _ => None,
        },
        1 => Some("CLOSE WINDOW"),
        2 => match r {
            0 => Some("OPEN FILES"),
            1 => Some("OPEN TERMINAL"),
            2 => Some("OPEN PAINT"),
            3 => Some("OPEN BROWSER"),
            5 => Some("CLOSE WINDOW"),
            _ => None,
        },
        3 => Some(gui::theme_name(r)),
        4 => unsafe {
            match WINS[r] {
                Some(w) => Some(apps::title(w.app)),
                None => None,
            }
        },
        5 => Some("OPEN MANUAL"),
        _ => None,
    }
}

/// does row r carry the active check mark?
fn menu_checked(m: i32, r: usize) -> bool {
    match m {
        3 => gui::theme() == r,
        4 => unsafe { r + 1 == N },
        _ => false,
    }
}

/// run the action behind row r of menu m (the menu is already closed)
fn run_item(m: i32, r: usize) {
    match m {
        0 => {
            if r == 0 {
                open(AppId::About);
            } else if r == 2 {
                open(AppId::Settings);
            }
        }
        1 => unsafe {
            if N > 0 {
                close(N - 1);
            }
        },
        2 => match r {
            0 => open(AppId::Files),
            1 => open(AppId::Terminal),
            2 => open(AppId::Paint),
            3 => open(AppId::Browser),
            5 => unsafe {
                if N > 0 {
                    close(N - 1);
                }
            },
            _ => {}
        },
        3 => {
            gui::set_theme(r);
            gui::bg_regen();
        }
        4 => unsafe { raise(r); },
        5 => {
            open(AppId::Browser);
            apps::show_manual(); // after open: on_open resets the page
        }
        _ => {}
    }
}

/// panel geometry of menu m: (x, y, width, rows)
fn menu_panel(m: i32) -> (i32, i32, i32, usize) {
    let n = menu_len(m);
    let mut w = 0i32;
    for r in 0..n {
        if let Some(s) = menu_label(m, r) {
            let tw = gui::text_w(s, 2);
            if tw > w {
                w = tw;
            }
        }
    }
    w += 56; // check column + right padding
    if w < 150 {
        w = 150;
    }
    let (tx, _) = menu_rect(m);
    (tx - 6, 30, w, n)
}

/// which menu title sits under the bar point (cx, cy)? -1 = none
fn menu_at(cx: i32, cy: i32) -> i32 {
    if cy > 28 {
        return -1;
    }
    for m in 0..MENU_N {
        let (x0, x1) = menu_rect(m);
        if cx >= x0 - 6 && cx < x1 + 6 {
            return m;
        }
    }
    -1
}

/// live hover while a menu is open: switch titles, track the item row
fn menu_hover(cx: i32, cy: i32) {
    unsafe {
        let m = MENU_OPEN;
        if m < 0 {
            return;
        }
        // hovering the bar switches menus (macOS behavior)
        let hit = menu_at(cx, cy);
        if hit >= 0 {
            if hit != m && menu_len(hit) > 0 {
                MENU_OPEN = hit;
                MENU_HOVER = -1;
                DIRTY = true;
            }
            return;
        }
        // hovering the panel highlights an item row
        let (px, py, pw, n) = menu_panel(m);
        let mut row = -1i32;
        if n > 0 && cx >= px && cx < px + pw && cy >= py && cy < py + n as i32 * 20 + 8 {
            let r = (cy - (py + 1)) / 20;
            if r >= 0 && r < n as i32 && menu_label(m, r as usize).is_some() {
                row = r;
            }
        }
        if row != MENU_HOVER {
            MENU_HOVER = row;
            DIRTY = true;
        }
    }
}

/// draw the open menu: highlighted title, dropdown panel, items
fn draw_menu(m: i32) {
    unsafe {
        let n = menu_len(m);
        if n == 0 {
            return;
        }
        // highlighted title in the bar
        let (tx0, tx1) = menu_rect(m);
        gui::round_rect_fill(tx0 - 6, 2, tx1 - tx0 + 12, 24, 6, ACCENT);
        if m == 0 {
            gui::draw_monogram();
        } else {
            gui::draw_text(tx0, 7, menu_title(m), 2, 0xFFFFFFFF);
        }
        // dropdown: shadow + panel
        let (px, py, pw, _) = menu_panel(m);
        let ph = n as i32 * 20 + 8;
        gui::round_rect_blend(px - 8, py, pw + 16, ph + 12, 14, 0x000000, 120);
        gui::round_rect_fill(px, py, pw, ph, 10, 0xFF1F1F26);
        gui::blend_rect(px + 3, py + 1, pw - 6, 1, 0xFFFFFF, 26);
        for r in 0..n {
            let iy = py + 3 + r as i32 * 20;
            match menu_label(m, r) {
                None => {
                    gui::blend_rect(px + 8, iy + 6, pw - 16, 1, 0xFFFFFF, 30);
                }
                Some(s) => {
                    let hov = MENU_HOVER == r as i32;
                    if hov {
                        gui::round_rect_fill(px + 5, iy - 2, pw - 10, 18, 5, ACCENT);
                    }
                    let col = if hov { 0xFFFFFFFF } else { 0xFFE8E8EC };
                    gui::draw_text(px + 26, iy, s, 2, col);
                    if menu_checked(m, r) {
                        // check mark: two short strokes
                        gui::draw_line_t(px + 10, iy + 5, px + 13, iy + 8, 2, col);
                        gui::draw_line_t(px + 13, iy + 8, px + 19, iy + 1, 2, col);
                    }
                }
            }
        }
    }
}

// left mouse button went down at (cx, cy)
fn press(cx: i32, cy: i32) {
    unsafe {
        // ---- menubar menus first ----
        if MENU_OPEN >= 0 {
            let m = MENU_OPEN;
            // click on the open panel: run the item (separators do nothing)
            let (px, py, pw, n) = menu_panel(m);
            if n > 0 && cx >= px && cx < px + pw && cy >= py && cy < py + n as i32 * 20 + 8 {
                let r = (cy - (py + 1)) / 20;
                if r >= 0 && r < n as i32 && menu_label(m, r as usize).is_some() {
                    MENU_OPEN = -1;
                    MENU_HOVER = -1;
                    DIRTY = true;
                    run_item(m, r as usize);
                }
                return;
            }
            // click on another title switches, on the same title toggles closed,
            // anywhere else closes and swallows the click (macOS behavior)
            let hit = menu_at(cx, cy);
            MENU_OPEN = if hit >= 0 && hit != m && menu_len(hit) > 0 {
                hit
            } else {
                -1
            };
            MENU_HOVER = -1;
            DIRTY = true;
            return;
        }
        let hit = menu_at(cx, cy);
        if hit >= 0 {
            if menu_len(hit) > 0 {
                MENU_OPEN = hit;
                MENU_HOVER = -1;
            }
            DIRTY = true;
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
        if !held {
            // menus are open: live title switching + item highlighting
            menu_hover(cx, cy);
        }
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
            // any key press closes an open menu first
            if MENU_OPEN >= 0 {
                MENU_OPEN = -1;
                MENU_HOVER = -1;
                DIRTY = true;
                continue;
            }
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

/// draw the static menubar titles over the background bar
fn draw_menu_titles() {
    gui::draw_text(38, 7, app_menu_name(), 2, 0xFFE8E8EC);
    let mut x = 38 + gui::text_w(app_menu_name(), 2) + 18;
    for t in MENU_TITLES {
        gui::draw_text(x, 7, t, 2, 0xFFC8C8D0);
        x += gui::text_w(t, 2) + 18;
    }
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
        // macOS-style menubar titles live over the background bar
        draw_menu_titles();
        // menus draw above every window; the cursor stays on top
        if MENU_OPEN >= 0 {
            draw_menu(MENU_OPEN);
        }
        gui::clear_clip();
        gui::draw_cursor();
    }
}
