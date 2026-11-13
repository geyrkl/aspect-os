// ASPECT OS - bundled apps: file manager (virtual FS), terminal, calculator,
// paint, settings (themes), offline browser, about. All state is static
// memory - no heap, no external code.
#![allow(unknown_lints, static_mut_refs, dead_code)]
use crate::gui;
use crate::pit;
use crate::pmm;
use crate::wm;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AppId {
    Files,
    Terminal,
    Calc,
    Paint,
    Settings,
    Browser,
    About,
}

pub fn title(app: AppId) -> &'static str {
    match app {
        AppId::Files => "FILES",
        AppId::Terminal => "TERMINAL",
        AppId::Calc => "CALCULATOR",
        AppId::Paint => "PAINT",
        AppId::Settings => "SETTINGS",
        AppId::Browser => "BROWSER",
        AppId::About => "ABOUT",
    }
}

pub fn default_size(app: AppId) -> (i32, i32) {
    match app {
        AppId::Files => (460, 320),
        AppId::Terminal => (560, 380),
        AppId::Calc => (300, 420),
        AppId::Paint => (992, 664),
        AppId::Settings => (460, 320),
        AppId::Browser => (660, 460),
        AppId::About => (440, 320),
    }
}

pub fn init() {
    unsafe {
        F_FOLDER = 0;
        F_SEL = -1;
        T_IN_LEN = 0;
        C_RESET();
        P_COLOR = 1; // black - the canvas starts white
        P_LAST = -1;
        B_PAGE = 0;
        B_URL_LEN = 0;
        B_EDIT = false;
    }
    paint_clear();
    term_clear();
    term_push_str("ASPECT OS TERMINAL");
    term_push_str("TYPE HELP FOR COMMANDS");
    term_push_str("");
}

pub fn on_open(app: AppId) {
    match app {
        AppId::Terminal => {
            term_clear();
            term_push_str("ASPECT OS TERMINAL");
            term_push_str("TYPE HELP FOR COMMANDS");
            term_push_str("");
        }
        _ => {}
    }
}

// ---- shared helpers ----

/// draw ASCII bytes with simple char wrapping; returns lines drawn
fn draw_wrap(x: i32, y: i32, b: &[u8], sc: i32, c: u32, max_w: i32) -> i32 {
    let max_chars = (max_w / (6 * sc)).max(1) as usize;
    let mut ry = y;
    let mut i = 0;
    while i < b.len() {
        let n = (b.len() - i).min(max_chars);
        gui::draw_text_b(x, ry, &b[i..i + n], sc, c);
        ry += 8 * sc;
        i += n;
    }
    (ry - y) / (8 * sc)
}

/// draw a multi-line string honoring \n and wrapping long lines
fn draw_multiline(x: i32, y: i32, s: &str, sc: i32, c: u32, max_w: i32) -> i32 {
    let mut ry = y;
    for line in s.split('\n') {
        ry += draw_wrap(x, ry, line.as_bytes(), sc, c, max_w) * (8 * sc);
    }
    ry
}

fn num_into(buf: &mut [u8], len: &mut usize, v: u64) {
    if *len < buf.len() {
        buf[*len] = b'0';
        *len += 1;
    }
    let mut digits = [0u8; 20];
    let mut n = 0;
    let mut x = v;
    if x == 0 {
        digits[0] = b'0';
        n = 1;
    }
    while x > 0 {
        digits[n] = b'0' + (x % 10) as u8;
        n += 1;
        x /= 10;
    }
    for i in (0..n).rev() {
        if *len < buf.len() {
            buf[*len] = digits[i];
            *len += 1;
        }
    }
}

// =====================================================================
// FILE MANAGER - backed by ASPFS on disk (real files, real folders)
// =====================================================================

const F_MAXD: usize = 3; // folder tabs (root subdirectories)
const F_MAXF: usize = 8; // files shown per folder

static mut F_FOLDER: usize = 0;
static mut F_SEL: i32 = -1;
static mut F_POP: bool = false; // file lists filled from disk
static mut F_DIR: [[u8; 64]; F_MAXD] = [[0; 64]; F_MAXD];
static mut F_FILES: [[[u8; 64]; F_MAXF]; F_MAXD] = [[[0; 64]; F_MAXF]; F_MAXD];
static mut F_COUNT: [usize; F_MAXD] = [0; F_MAXD];
static mut F_TEXT: [u8; 2048] = [0; 2048]; // content of the selected file
static mut F_TLEN: usize = 0;
static mut F_SRC: i32 = -1; // folder * F_MAXF + sel of loaded text

/// fill the folder/file lists from the mounted file system
pub fn fs_populate() {
    unsafe {
        F_POP = false;
        F_SEL = -1;
        F_SRC = -1;
        F_TLEN = 0;
        if !crate::fs::ready() {
            return;
        }
        for d in 0..F_MAXD {
            if d >= crate::fs::dir_count() {
                break;
            }
            let mut name = [0u8; 64];
            let n = crate::fs::dir_name(d, &mut name);
            F_DIR[d] = [0u8; 64];
            F_DIR[d][..n].copy_from_slice(&name[..n]);
            F_COUNT[d] = crate::fs::list_dir(d, &mut F_FILES[d], F_MAXF);
        }
        F_POP = true;
    }
}

/// length of a zero-terminated name buffer
fn nlen(b: &[u8]) -> usize {
    let mut n = 0usize;
    while n < b.len() && b[n] != 0 {
        n += 1;
    }
    n
}

/// draw a byte buffer as ascii
fn draw_name(x: i32, y: i32, b: &[u8], sc: i32, c: u32) {
    let n = nlen(b);
    gui::draw_text_b(x, y, &b[..n], sc, c);
}

/// multi-line byte content (splits on \n, wraps long lines)
fn draw_multiline_b(x: i32, y: i32, b: &[u8], sc: i32, c: u32, max_w: i32) -> i32 {
    let mut ry = y;
    let mut start = 0usize;
    for i in 0..=b.len() {
        if i == b.len() || b[i] == b'\n' {
            ry += draw_wrap(x, ry, &b[start..i], sc, c, max_w) * (8 * sc);
            start = i + 1;
        }
    }
    ry
}

fn files_draw(x: i32, y: i32, w: i32, h: i32) {
    gui::fill_rect(x, y, w, h, 0xFF202028);
    unsafe {
        if !F_POP || !crate::fs::ready() {
            gui::draw_text(x + 12, y + 60, "NO DISK - FILE SYSTEM", 2, 0xFF8A8A96);
            gui::draw_text(x + 12, y + 78, "NOT MOUNTED", 2, 0xFF8A8A96);
            return;
        }
        // folder tabs
        for f in 0..F_MAXD {
            let bx = x + 8 + f as i32 * 104;
            let sel = F_FOLDER == f;
            gui::fill_rect(bx, y + 8, 96, 26, if sel { 0xFF3E3E4A } else { 0xFF2A2A34 });
            let tw = gui::text_w_b(&F_DIR[f], 2);
            gui::draw_text_b(bx + (96 - tw) / 2, y + 15, &F_DIR[f], 2, if sel { 0xFFFFFFFF } else { 0xFF9A9AA6 });
        }
        // file list (left pane)
        gui::fill_rect(x + 8, y + 44, 168, h - 52, 0xFF1A1A20);
        let folder = F_FOLDER;
        for i in 0..F_COUNT[folder] {
            let ry = y + 50 + i as i32 * 26;
            let sel = F_SEL == i as i32;
            if sel {
                gui::fill_rect(x + 12, ry - 3, 160, 22, 0xFF3E5E8E);
            }
            draw_name(x + 18, ry, &F_FILES[folder][i], 2, if sel { 0xFFFFFFFF } else { 0xFFB0B0BC });
        }
        // divider
        gui::fill_rect(x + 184, y + 44, 2, h - 52, 0xFF33333E);
        // detail pane (right)
        let dx = x + 196;
        let dw = x + w - dx - 8;
        let mut sy = y + 50;
        if F_SEL < 0 {
            gui::draw_text(dx, sy, "SELECT A FILE", 2, 0xFF8A8A96);
        } else {
            let f = F_SEL as usize;
            if f >= F_COUNT[folder] {
                return;
            }
            draw_name(dx, sy, &F_FILES[folder][f], 2, 0xFFFFFFFF);
            sy += 26;
            gui::fill_rect(dx, sy, dw, 1, 0xFF33333E);
            sy += 10;
            if F_SRC == (folder * F_MAXF + f) as i32 {
                draw_multiline_b(dx, sy, &F_TEXT[..F_TLEN], 2, 0xFFC0C0CA, dw);
            } else {
                gui::draw_text(dx, sy, "...", 2, 0xFF8A8A96);
            }
        }
    }
}

fn files_click(lx: i32, ly: i32) {
    unsafe {
        // folder tabs
        if ly >= 8 && ly < 34 {
            for f in 0..F_MAXD {
                let bx = 8 + f as i32 * 104;
                if lx >= bx && lx < bx + 96 {
                    F_FOLDER = f;
                    F_SEL = -1;
                    return;
                }
            }
        }
        // file rows: selecting loads the content from disk
        if lx >= 8 && lx < 176 && ly >= 44 {
            let row = (ly - 44 - 6) / 26;
            if row >= 0 && (row as usize) < F_COUNT[F_FOLDER] {
                F_SEL = row;
                let f = row as usize;
                let mut name = [0u8; 64];
                let n = nlen(&F_FILES[F_FOLDER][f]);
                name[..n].copy_from_slice(&F_FILES[F_FOLDER][f][..n]);
                let got = crate::fs::read_file(F_FOLDER, &name[..n], &mut F_TEXT);
                F_TLEN = got;
                F_SRC = (F_FOLDER * F_MAXF + f) as i32;
            }
        }
    }
}

fn files_key(code: u32) {
    unsafe {
        if code == 258 {
            F_SEL = -1; // escape deselects
        }
    }
}

// =====================================================================
// TERMINAL - text grid, static buffers
// =====================================================================

const T_COLS: usize = 38;
const T_ROWS: usize = 13;

static mut T_BUF: [[u8; T_COLS]; T_ROWS] = [[0; T_COLS]; T_ROWS];
static mut T_IN: [u8; T_COLS] = [0; T_COLS];
static mut T_IN_LEN: usize = 0;
static mut T_USED: usize = 0; // rows filled (scrolls when full)
static mut CATB: [u8; 2048] = [0; 2048]; // CAT file content staging

fn term_clear() {
    unsafe {
        for r in 0..T_ROWS {
            T_BUF[r] = [0; T_COLS];
        }
        T_IN_LEN = 0;
        T_USED = 0;
    }
}

/// append one line at the cursor row, scrolling when the screen is full
fn term_push(line: &[u8]) {
    unsafe {
        if T_USED == T_ROWS {
            for r in 0..T_ROWS - 1 {
                T_BUF[r] = T_BUF[r + 1];
            }
            T_USED = T_ROWS - 1;
        }
        let mut row = [0u8; T_COLS];
        let n = line.len().min(T_COLS);
        row[..n].copy_from_slice(&line[..n]);
        T_BUF[T_USED] = row;
        T_USED += 1;
    }
}

fn term_push_str(s: &str) {
    term_push(s.as_bytes());
}

/// build a line from a string + a number
fn term_push_num(s: &str, v: u64, suffix: &str) {
    let mut buf = [0u8; T_COLS];
    let mut len = 0;
    for &b in s.as_bytes() {
        if len < buf.len() {
            buf[len] = b;
            len += 1;
        }
    }
    num_into(&mut buf, &mut len, v);
    for &b in suffix.as_bytes() {
        if len < buf.len() {
            buf[len] = b;
            len += 1;
        }
    }
    term_push(&buf[..len]);
}

fn term_exec(cmd: &[u8]) {
    // split "word rest"
    let mut word_end = 0;
    while word_end < cmd.len() && cmd[word_end] != b' ' {
        word_end += 1;
    }
    let word = &cmd[..word_end];
    let rest = if word_end < cmd.len() { &cmd[word_end + 1..] } else { &cmd[0..0] };

    // uppercase compare
    let mut upper = [0u8; T_COLS];
    let ulen = word.len().min(T_COLS);
    for i in 0..ulen {
        upper[i] = word[i].to_ascii_uppercase();
    }
    let w = &upper[..ulen];

    let eq = |s: &str| w == s.as_bytes();
    if eq("HELP") {
        term_push_str("COMMANDS:");
        term_push_str("HELP CLEAR ECHO VER");
        term_push_str("UPTIME MEM APPS ABOUT");
        term_push_str("LS CAT WRITE RM DF");
        term_push_str("IP PING");    } else if eq("CLEAR") {
        term_clear();
    } else if eq("ECHO") {
        term_push(rest);
    } else if eq("VER") {
        term_push_str("ASPECT OS 0.5.0 M5");
    } else if eq("UPTIME") {
        term_push_num("UP ", pit::ticks() / 1000, " SECONDS");
    } else if eq("MEM") {
        term_push_num("MEM: ", pmm::used_frames(), " FRAMES IN USE");
    } else if eq("APPS") {
        term_push_num("WINDOWS OPEN: ", wm::count() as u64, "");
    } else if eq("ABOUT") {
        term_push_str("ASPECT OS - OWN RUST OS");
        term_push_str("BOOTLOADER KERNEL GUI APPS");
    } else if eq("DF") {
        if crate::fs::ready() {
            term_push_num("DISK FREE: ", crate::fs::free_kb() as u64, " KB");
        } else {
            term_push_str("NO DISK");
        }
    } else if eq("LS") {
        term_ls(rest);
    } else if eq("CAT") {
        term_cat(rest);
    } else if eq("WRITE") {
        term_write(rest);
    } else if eq("RM") {
        term_rm(rest);
    } else if eq("IP") {
        term_ip();
    } else if eq("PING") {
        term_ping(rest);
    } else if ulen == 0 {
        // empty command: just a new prompt line
    } else {
        term_push(b"UNKNOWN COMMAND");
    }
}

/// resolve a "DIR/NAME" path: uppercase compare against the root dirs;
/// returns (dir_index, file_name_slice_end) or None
fn term_path(arg: &[u8]) -> Option<(usize, usize)> {
    // uppercase copy
    let mut up = [0u8; 62];
    for i in 0..arg.len().min(62) {
        up[i] = arg[i].to_ascii_uppercase();
    }
    // split at '/'
    let mut slash = arg.len();
    for i in 0..arg.len() {
        if arg[i] == b'/' || arg[i] == b'\\' {
            slash = i;
            break;
        }
    }
    if slash == arg.len() {
        return None;
    }
    let dirname = &up[..slash];
    let filename = slash + 1; // index into `arg` (original case)
    if filename >= arg.len() {
        return None;
    }
    unsafe {
        for d in 0..crate::fs::dir_count() {
            let mut n = 0usize;
            while n < 64 && F_DIR[d][n] != 0 {
                n += 1;
            }
            if n == dirname.len() {
                let mut ok = true;
                for i in 0..n {
                    if F_DIR[d][i].to_ascii_uppercase() != dirname[i] {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    return Some((d, filename));
                }
            }
        }
    }
    None
}

/// LS [DIR] - list files (empty arg: summary of all folders)
fn term_ls(arg: &[u8]) {
    if !crate::fs::ready() {
        term_push_str("NO DISK");
        return;
    }
    if arg.is_empty() {
        unsafe {
            for d in 0..crate::fs::dir_count() {
                let mut n = 0usize;
                while n < 64 && F_DIR[d][n] != 0 {
                    n += 1;
                }
                let mut line = [0u8; 38];
                let mut L = 0;
                for i in 0..n {
                    if L < 38 {
                        line[L] = F_DIR[d][i];
                        L += 1;
                    }
                }
                let suffix = format_count(F_COUNT[d]);
                for &b in suffix {
                    if L < 38 {
                        line[L] = b;
                        L += 1;
                    }
                }
                term_push(&line[..L]);
            }
        }
        return;
    }
    let mut up = [0u8; 62];
    for i in 0..arg.len().min(62) {
        up[i] = arg[i].to_ascii_uppercase();
    }
    let up = &up[..arg.len().min(62)];
    unsafe {
        for d in 0..crate::fs::dir_count() {
            let n = nlen(&F_DIR[d]);
            if n == up.len() && {
                let mut ok = true;
                for i in 0..n {
                    if F_DIR[d][i] != up[i] {
                        ok = false;
                        break;
                    }
                }
                ok
            } {
                if F_COUNT[d] == 0 {
                    term_push_str("EMPTY FOLDER");
                }
                for i in 0..F_COUNT[d] {
                    term_push(&F_FILES[d][i][..nlen(&F_FILES[d][i])]);
                }
                return;
            }
        }
    }
    term_push_str("NO SUCH FOLDER");
}

/// CAT DIR/NAME - print a file from disk
fn term_cat(arg: &[u8]) {
    if !crate::fs::ready() {
        term_push_str("NO DISK");
        return;
    }
    let p = term_path(arg);
    if p.is_none() {
        term_push_str("USAGE: CAT DIR/NAME");
        return;
    }
    let (d, fs_) = p.unwrap();
    // file name = arg[fs_..] up to the next space
    let mut end = fs_;
    while end < arg.len() && arg[end] != b' ' {
        end += 1;
    }
    let name = &arg[fs_..end];
    unsafe {
        let got = crate::fs::read_file(d, name, &mut CATB);
        if got == 0 {
            term_push_str("NO SUCH FILE");
            return;
        }
        let mut start = 0usize;
        for i in 0..=got {
            if i == got || CATB[i] == b'\n' {
                // wrap long lines at the terminal width
                let mut s = start;
                while s < i {
                    let n = (i - s).min(T_COLS);
                    term_push(&CATB[s..s + n]);
                    s += n;
                }
                start = i + 1;
            }
        }
    }
}

/// WRITE DIR/NAME TEXT - create or overwrite a file on disk
fn term_write(arg: &[u8]) {
    if !crate::fs::ready() {
        term_push_str("NO DISK");
        return;
    }
    let p = term_path(arg);
    if p.is_none() {
        term_push_str("USAGE: WRITE DIR/NAME TEXT");
        return;
    }
    let (d, fs_) = p.unwrap();
    let mut end = fs_;
    while end < arg.len() && arg[end] != b' ' {
        end += 1;
    }
    if end == fs_ {
        term_push_str("USAGE: WRITE DIR/NAME TEXT");
        return;
    }
    let name = &arg[fs_..end];
    let mut upname = [0u8; 62];
    let nlen2 = name.len().min(62);
    for i in 0..nlen2 {
        upname[i] = name[i].to_ascii_uppercase();
    }
    let text = if end < arg.len() { &arg[end + 1..] } else { &arg[0..0] };
    if crate::fs::write_file(d, &upname[..nlen2], text) {
        term_push_str("FILE WRITTEN TO DISK");
        // refresh the file manager lists so the new file appears
        fs_populate();
    } else {
        term_push_str("WRITE FAILED");
    }
}

/// RM DIR/NAME - delete a file from disk
fn term_rm(arg: &[u8]) {
    if !crate::fs::ready() {
        term_push_str("NO DISK");
        return;
    }
    let p = term_path(arg);
    if p.is_none() {
        term_push_str("USAGE: RM DIR/NAME");
        return;
    }
    let (d, fs_) = p.unwrap();
    let mut end = fs_;
    while end < arg.len() && arg[end] != b' ' {
        end += 1;
    }
    let name = &arg[fs_..end];
    if crate::fs::delete_file(d, name) {
        term_push_str("FILE DELETED");
        fs_populate();
    } else {
        term_push_str("NO SUCH FILE");
    }
}

/// IP - show the network configuration
fn term_ip() {
    if !crate::net::ready() {
        term_push_str("NO NETWORK HARDWARE");
        return;
    }
    let mut buf = [0u8; 40];
    let mut line = [0u8; 40];
    unsafe {
        // MAC
        let mac = crate::e1000::mac();
        let n = crate::net::mac_str(&mac, &mut buf);
        line[0] = b'M';
        line[1] = b'A';
        line[2] = b'C';
        line[3] = b':';
        line[4] = b' ';
        let mut L = 5;
        for i in 0..n {
            if L < 40 {
                line[L] = buf[i];
                L += 1;
            }
        }
        term_push(&line[..L]);
        // IP
        let n = crate::net::ip_str(&crate::net::OUR_IP, &mut buf);
        line[0] = b'I';
        line[1] = b'P';
        line[2] = b':';
        line[3] = b' ';
        L = 4;
        for i in 0..n {
            if L < 40 {
                line[L] = buf[i];
                L += 1;
            }
        }
        term_push(&line[..L]);
        // gateway
        let n = crate::net::ip_str(&crate::net::GW_IP, &mut buf);
        line[0] = b'G';
        line[1] = b'W';
        line[2] = b':';
        line[3] = b' ';
        L = 4;
        for i in 0..n {
            if L < 40 {
                line[L] = buf[i];
                L += 1;
            }
        }
        term_push(&line[..L]);
        let (rx, tx) = crate::net::stats();
        // one line: "PACKETS RX n TX m"
        line[0] = b'P';
        line[1] = b'A';
        line[2] = b'C';
        line[3] = b'K';
        line[4] = b'E';
        line[5] = b'T';
        line[6] = b'S';
        line[7] = b' ';
        line[8] = b'R';
        line[9] = b'X';
        line[10] = b' ';
        let mut L = 11usize;
        let mut put = |v: u32, tail: &[u8]| {
            let mut d = [0u8; 10];
            let mut n = 0usize;
            let mut x = v;
            loop {
                d[n] = b'0' + (x % 10) as u8;
                n += 1;
                x /= 10;
                if x == 0 {
                    break;
                }
            }
            while n > 0 {
                n -= 1;
                if L < 38 {
                    line[L] = d[n];
                    L += 1;
                }
            }
            for &c in tail {
                if L < 38 {
                    line[L] = c;
                    L += 1;
                }
            }
        };
        put(rx, b" TX ");
        put(tx, b"");
        term_push(&line[..L]);
    }
}

/// PING [X.Y.Z.W] - send 4 ICMP echo requests (default: the gateway)
fn term_ping(arg: &[u8]) {
    if !crate::net::ready() {
        term_push_str("NO NETWORK HARDWARE");
        return;
    }
    // parse an optional dotted address
    let mut target = crate::net::GW_IP;
    let mut found = [0u8; 4];
    let mut cur = 0usize;
    let mut val = 0u32;
    let mut digits = 0usize;
    let mut any = false;
    for &c in arg {
        if c.is_ascii_digit() {
            val = val * 10 + (c - b'0') as u32;
            digits += 1;
        } else if c == b'.' && digits > 0 && val <= 255 {
            if cur < 4 {
                found[cur] = val as u8;
            }
            cur += 1;
            val = 0;
            digits = 0;
            any = true;
        } else {
            break;
        }
    }
    if any && cur == 3 && digits > 0 && val <= 255 {
        found[3] = val as u8;
        target = found;
    }
    unsafe {
        let mut tb = [0u8; 20];
        let n = crate::net::ip_str(&target, &mut tb);
        let mut head = [0u8; 24];
        let mut h = 0usize;
        for &b in b"PING " {
            head[h] = b;
            h += 1;
        }
        for i in 0..n {
            head[h] = tb[i];
            h += 1;
        }
        term_push(&head[..h]);
        for _ in 0..4 {
            match crate::net::ping_once(&target) {
                Some(rtt) => {
                    term_push_num("REPLY RTT ", rtt, " MS");
                }
                None => {
                    term_push_str("TIMEOUT");
                }
            }
        }
    }
}

/// suffix for the LS summary: " X FILES"
fn format_count(n: usize) -> &'static [u8] {
    match n {
        0 => b": EMPTY",
        1 => b": 1 FILE",
        2 => b": 2 FILES",
        3 => b": 3 FILES",
        4 => b": 4 FILES",
        5 => b": 5 FILES",
        6 => b": 6 FILES",
        7 => b": 7 FILES",
        _ => b": 8 FILES",
    }
}

fn terminal_draw(x: i32, y: i32, w: i32, h: i32) {
    gui::fill_rect(x, y, w, h, 0xFF14141A);
    unsafe {
        for r in 0..T_ROWS {
            let line_len = {
                let row = &T_BUF[r];
                let mut n = T_COLS;
                while n > 0 && row[n - 1] == 0 {
                    n -= 1;
                }
                n
            };
            if line_len > 0 {
                gui::draw_text_b(x + 8, y + 8 + r as i32 * 16, &T_BUF[r][..line_len], 2, 0xFFC8E8C8);
            }
        }
        // prompt line
        let py = y + 8 + T_ROWS as i32 * 16;
        gui::draw_text(x + 8, py, "> ", 2, 0xFF8AE88A);
        gui::draw_text_b(x + 8 + 24, py, &T_IN[..T_IN_LEN], 2, 0xFFE8E8EC);
        gui::fill_rect(x + 8 + 24 + gui::text_w_b(&T_IN[..T_IN_LEN], 2), py, 10, 14, 0xFF8AE88A);
    }
}

fn terminal_key(code: u32) {
    unsafe {
        match code {
            256 => {
                // Enter: execute and clear input
                let n = T_IN_LEN;
                let mut cmd = [0u8; T_COLS];
                cmd[..n].copy_from_slice(&T_IN[..n]);
                T_IN_LEN = 0;
                term_exec(&cmd[..n]);
            }
            257 => {
                if T_IN_LEN > 0 {
                    T_IN_LEN -= 1;
                    T_IN[T_IN_LEN] = 0;
                }
            }
            258 => {
                T_IN_LEN = 0;
            }
            c if (32..126).contains(&c) => {
                if T_IN_LEN < T_IN.len() {
                    T_IN[T_IN_LEN] = c as u8;
                    T_IN_LEN += 1;
                }
            }
            _ => {}
        }
    }
}

// =====================================================================
// CALCULATOR - i64 integer math
// =====================================================================

static mut C_ACC: i64 = 0;
static mut C_OP: u8 = 0; // 0 = none, else '+', '-', '*', '/'
static mut C_CUR: i64 = 0;
static mut C_ENT: bool = false;
static mut C_ERR: bool = false;

fn C_RESET() {
    unsafe {
        C_ACC = 0;
        C_OP = 0;
        C_CUR = 0;
        C_ENT = false;
        C_ERR = false;
    }
}

fn calc_digit(d: i64) {
    unsafe {
        if C_ERR {
            C_RESET();
        }
        if C_ENT {
            C_CUR = C_CUR.wrapping_mul(10).wrapping_add(d);
        } else {
            C_CUR = d;
            C_ENT = true;
        }
    }
}

fn calc_op(op: u8) {
    unsafe {
        if C_ERR {
            return;
        }
        if C_OP != 0 {
            if C_ENT {
                if C_OP == b'/' && C_CUR == 0 {
                    C_ERR = true;
                    return;
                }
                C_CUR = calc_apply(C_ACC, C_OP, C_CUR);
            }
        }
        C_ACC = C_CUR;
        C_OP = op;
        C_ENT = false;
    }
}

fn calc_eq() {
    unsafe {
        if C_ERR || C_OP == 0 {
            return;
        }
        if C_OP == b'/' && C_CUR == 0 {
            C_ERR = true;
            C_OP = 0;
            return;
        }
        C_CUR = calc_apply(C_ACC, C_OP, C_CUR);
        C_OP = 0;
        C_ENT = false;
    }
}

fn calc_apply(a: i64, op: u8, b: i64) -> i64 {
    match op {
        b'+' => a.wrapping_add(b),
        b'-' => a.wrapping_sub(b),
        b'*' => a.wrapping_mul(b),
        _ => a.wrapping_div(b),
    }
}

// button grid: 4x4, cell 64x56, gap 8
const CALC_KEYS: [u8; 16] = [
    b'7', b'8', b'9', b'/',
    b'4', b'5', b'6', b'*',
    b'1', b'2', b'3', b'-',
    b'0', b'C', b'=', b'+',
];

fn calc_press_key(k: u8) {
    match k {
        b'0'..=b'9' => calc_digit((k - b'0') as i64),
        b'+' | b'-' | b'*' | b'/' => calc_op(k),
        b'=' => calc_eq(),
        _ => C_RESET(),
    }
}

fn calc_display(buf: &mut [u8], len: &mut usize) {
    unsafe {
        if C_ERR {
            for &b in b"ERROR" {
                buf[*len] = b;
                *len += 1;
            }
            return;
        }
        let v = if C_OP != 0 && !C_ENT { C_ACC } else { C_CUR };
        if v < 0 {
            buf[*len] = b'-';
            *len += 1;
            num_into(buf, len, (v as i128).unsigned_abs() as u64);
        } else {
            num_into(buf, len, v as u64);
        }
    }
}

fn calc_draw(x: i32, y: i32, w: i32, h: i32) {
    gui::fill_rect(x, y, w, h, 0xFF202028);
    // display
    gui::fill_rect(x + 8, y + 8, 280, 44, 0xFF16161C);
    let mut buf = [0u8; 24];
    let mut len = 0;
    calc_display(&mut buf, &mut len);
    let dw = gui::text_w_b(&buf[..len], 2);
    gui::draw_text_b(x + 8 + 280 - dw - 8, y + 22, &buf[..len], 2, 0xFFE8E8EC);
    // button grid
    for r in 0..4 {
        for c in 0..4 {
            let bx = x + 8 + c as i32 * 72;
            let by = y + 64 + r as i32 * 64;
            let k = CALC_KEYS[r * 4 + c];
            let accent = matches!(k, b'+' | b'-' | b'*' | b'/' | b'=');
            let bgc = if k == b'C' {
                0xFF7A2E38
            } else if accent {
                0xFF3E5E8E
            } else {
                0xFF2E2E38
            };
            gui::fill_rect(bx, by, 64, 56, bgc);
            let lbl = [k];
            let tw = gui::text_w_b(&lbl, 2);
            gui::draw_text_b(bx + (64 - tw) / 2, by + 21, &lbl, 2, 0xFFE8E8EC);
        }
    }
}

fn calc_click(lx: i32, ly: i32) {
    if lx >= 8 && lx < 288 && ly >= 64 && ly < 312 {
        let c = ((lx - 8) / 72) as usize;
        let r = ((ly - 64) / 64) as usize;
        calc_press_key(CALC_KEYS[r * 4 + c]);
    }
}

fn calc_key(code: u32) {
    match code {
        256 => calc_eq(),           // Enter
        257 => C_RESET(),           // Backspace clears
        c if (48..58).contains(&c) => calc_digit((c - 48) as i64),
        c if c == b'+' as u32 || c == b'-' as u32 || c == b'*' as u32 || c == b'/' as u32 => {
            calc_op(c as u8)
        }
        _ => {}
    }
}

// =====================================================================
// PAINT - fixed canvas buffer, palette, brush strokes
// =====================================================================

pub const CANVAS_W: i32 = 960;
pub const CANVAS_H: i32 = 560;
static mut CANVAS: [u32; (CANVAS_W * CANVAS_H) as usize] = [0; (CANVAS_W * CANVAS_H) as usize];

const PALETTE: [u32; 8] = [
    0xFFFFFFFF, 0xFF22222A, 0xFFE5484D, 0xFFF5A524,
    0xFFFFD53D, 0xFF46A758, 0xFF3E9EEA, 0xFF9E6BDE,
];
static mut P_COLOR: usize = 1; // default: black (index 1) - canvas starts white
static mut P_LAST: i32 = -1; // last stroke point packed as y*10000+x, -1 = none
static mut P_LAST_X: i32 = 0;
static mut P_LAST_Y: i32 = 0;

fn paint_clear() {
    unsafe {
        CANVAS = [0xFFFFFFFF; (CANVAS_W * CANVAS_H) as usize];
        P_LAST = -1;
    }
}

const CANVAS_OX: i32 = 12;
const CANVAS_OY: i32 = 8;

/// draw on the canvas buffer (own target, own clip)
fn paint_stroke(x0: i32, y0: i32, x1: i32, y1: i32) {
    unsafe {
        let color = PALETTE[P_COLOR];
        gui::set_target(CANVAS.as_mut_ptr(), CANVAS_W, CANVAS_H, CANVAS_W);
        gui::draw_line_t(x0, y0, x1, y1, 5, color);
        gui::set_target_fb();
    }
}

fn paint_in_canvas(lx: i32, ly: i32) -> bool {
    lx >= CANVAS_OX && lx < CANVAS_OX + CANVAS_W && ly >= CANVAS_OY && ly < CANVAS_OY + CANVAS_H
}

fn paint_draw(x: i32, y: i32, w: i32, h: i32) {
    gui::fill_rect(x, y, w, h, 0xFF23232B);
    // canvas (1:1 blit, clipped by the window)
    unsafe {
        for cy in 0..CANVAS_H {
            let row = cy as usize * CANVAS_W as usize;
            for cx in 0..CANVAS_W {
                gui::pset(x + CANVAS_OX + cx, y + CANVAS_OY + cy, CANVAS[row + cx as usize]);
            }
        }
    }
    // palette
    let py = y + CANVAS_OY + CANVAS_H + 10;
    for i in 0..PALETTE.len() {
        let sx = x + CANVAS_OX + i as i32 * 44;
        gui::fill_rect(sx, py, 32, 32, PALETTE[i]);
        if unsafe { P_COLOR } == i {
            gui::round_rect_stroke(sx - 3, py - 3, 38, 38, 8, 2, 0xFFFFFFFF);
        }
    }
    // clear button
    let bx = x + CANVAS_OX + PALETTE.len() as i32 * 44 + 8;
    gui::fill_rect(bx, py, 80, 32, 0xFF2E2E38);
    let tw = gui::text_w("CLEAR", 2);
    gui::draw_text(bx + (80 - tw) / 2, py + 9, "CLEAR", 2, 0xFFE8E8EC);
}

fn paint_click(lx: i32, ly: i32) {
    unsafe {
        if paint_in_canvas(lx, ly) {
            let cx = lx - CANVAS_OX;
            let cy = ly - CANVAS_OY;
            paint_stroke(cx, cy, cx, cy);
            P_LAST = 1;
            P_LAST_X = cx;
            P_LAST_Y = cy;
            return;
        }
        P_LAST = -1;
        let py = CANVAS_OY + CANVAS_H + 10;
        if ly >= py && ly < py + 32 {
            // palette swatches
            if lx >= CANVAS_OX && lx < CANVAS_OX + PALETTE.len() as i32 * 44 {
                let idx = ((lx - CANVAS_OX) / 44) as usize;
                if idx < PALETTE.len() {
                    P_COLOR = idx;
                }
                return;
            }
            // clear button
            let bx = CANVAS_OX + PALETTE.len() as i32 * 44 + 8;
            if lx >= bx && lx < bx + 80 {
                paint_clear();
            }
        }
    }
}

fn paint_move(lx: i32, ly: i32) {
    unsafe {
        if !paint_in_canvas(lx, ly) {
            P_LAST = -1;
            return;
        }
        let cx = lx - CANVAS_OX;
        let cy = ly - CANVAS_OY;
        if P_LAST == 1 {
            paint_stroke(P_LAST_X, P_LAST_Y, cx, cy);
        } else {
            paint_stroke(cx, cy, cx, cy);
        }
        P_LAST = 1;
        P_LAST_X = cx;
        P_LAST_Y = cy;
    }
}

// =====================================================================
// SETTINGS - theme picker (regenerates the desktop background)
// =====================================================================

fn settings_draw(x: i32, y: i32, w: i32, h: i32) {
    gui::fill_rect(x, y, w, h, 0xFF202028);
    gui::draw_text(x + 16, y + 14, "APPEARANCE", 3, 0xFFE8E8EC);
    gui::draw_text(x + 16, y + 46, "PICK A DESKTOP THEME", 1, 0xFF9A9AA6);
    for i in 0..gui::theme_count() {
        let ry = y + 66 + i as i32 * 52;
        let sel = gui::theme() == i;
        if sel {
            gui::fill_rect(x + 10, ry - 4, w - 20, 44, 0xFF3E5E8E);
        }
        // swatch: preview from the theme gradient at this screen position
        gui::fill_rect(x + 18, ry + 4, 32, 16, gui::gradient_color(x + 18, ry + 4));
        let name = gui::theme_name(i);
        gui::draw_text(x + 60, ry + 4, name, 2, if sel { 0xFFFFFFFF } else { 0xFFB0B0BC });
        if sel {
            gui::draw_text(x + 60 + gui::text_w(name, 2) + 12, ry + 4, "ACTIVE", 2, 0xFF8AE88A);
        }
    }
    gui::draw_text(x + 16, y + h - 12, "CLICK A THEME TO APPLY IT", 1, 0xFF8A8A96);
}

fn settings_click(lx: i32, ly: i32) {
    unsafe {
        if lx >= 10 && ly >= 62 {
            let row = (ly - 62) / 52;
            if row >= 0 && row < gui::theme_count() as i32 {
                gui::set_theme(row as usize);
                gui::bg_regen();
            }
        }
    }
}

// =====================================================================
// BROWSER - offline pages: HOME, ABOUT, MANUAL, 404
// =====================================================================

const B_HOME: usize = 0;
const B_ABOUT: usize = 1;
const B_MANUAL: usize = 2;
const B_NF: usize = 3;

static mut B_PAGE: usize = 0;
static mut B_URL: [u8; 24] = [0; 24];
static mut B_URL_LEN: usize = 0;
static mut B_EDIT: bool = false;

const URLBAR_H: i32 = 32;
const PAGE_Y: i32 = 48;

fn browser_draw(x: i32, y: i32, w: i32, h: i32) {
    gui::fill_rect(x, y, w, h, 0xFF202028);
    // URL bar
    gui::fill_rect(x + 8, y + 8, w - 16, URLBAR_H, 0xFF16161C);
    gui::draw_text(x + 16, y + 15, "URL", 2, 0xFF8A8A96);
    unsafe {
        gui::draw_text_b(x + 56, y + 15, &B_URL[..B_URL_LEN], 2, 0xFFE8E8EC);
        if B_EDIT {
            let tw = gui::text_w_b(&B_URL[..B_URL_LEN], 2);
            gui::fill_rect(x + 56 + tw, y + 15, 10, 14, 0xFF8AE88A);
        }
    }
    let page = unsafe { B_PAGE };
    let px = x + 24;
    let pw = w - 48;
    let py = y + PAGE_Y;
    match page {
        B_HOME => {
            gui::draw_text(px, py + 16, "ASPECT OS", 4, 0xFFFFFFFF);
            gui::draw_text(px, py + 52, "OFFLINE BROWSER - MILESTONE 4", 2, 0xFF9A9AA6);
            gui::fill_rect(px, py + 76, pw, 1, 0xFF33333E);
            let links: [&str; 2] = ["ABOUT ASPECT OS", "USER MANUAL"];
            for i in 0..links.len() {
                let ly = py + 100 + i as i32 * 40;
                gui::draw_text(px, ly, links[i], 2, 0xFF6CB6FF);
                let lw = gui::text_w(links[i], 2);
                gui::fill_rect(px, ly + 16, lw, 1, 0xFF6CB6FF);
            }
        }
        B_ABOUT => {
            gui::draw_text(px, py, "ABOUT", 3, 0xFFFFFFFF);
            draw_multiline(px, py + 34,
                "ASPECT OS IS A DESKTOP\nOPERATING SYSTEM BUILT\nCOMPLETELY FROM SCRATCH\nIN RUST.\n\nNO LINUX NO GRUB NO\nEXTERNAL CODE.", 2, 0xFFC0C0CA, pw);
        }
        B_MANUAL => {
            gui::draw_text(px, py, "USER MANUAL", 3, 0xFFFFFFFF);
            draw_multiline(px, py + 34,
                "DOCK: CLICK AN ICON TO\nOPEN AN APP.\n\nWINDOWS: DRAG THE TITLE\nBAR TO MOVE. RED DOT\nCLOSES.\n\nTERMINAL: TYPE HELP.", 2, 0xFFC0C0CA, pw);
        }
        _ => {
            gui::draw_text(px, py + 16, "404", 4, 0xFFE5484D);
            gui::draw_text(px, py + 52, "PAGE NOT FOUND", 2, 0xFF9A9AA6);
            unsafe {
                gui::draw_text_b(px, py + 76, b"URL: ", 2, 0xFF9A9AA6);
                gui::draw_text_b(px + 56, py + 76, &B_URL[..B_URL_LEN], 2, 0xFFE8E8EC);
            }
        }
    }
}

fn browser_navigate() {
    unsafe {
        let mut up = [0u8; 24];
        for i in 0..B_URL_LEN {
            up[i] = B_URL[i].to_ascii_uppercase();
        }
        let u = &up[..B_URL_LEN];
        let eq = |s: &str| u == s.as_bytes();
        B_PAGE = if eq("HOME") || eq("ASPECT") || eq("ASPECT OS") || B_URL_LEN == 0 {
            B_HOME
        } else if eq("ABOUT") {
            B_ABOUT
        } else if eq("MANUAL") || eq("HELP") {
            B_MANUAL
        } else {
            B_NF
        };
    }
}

fn browser_click(lx: i32, ly: i32) {
    unsafe {
        // URL bar
        if ly >= 8 && ly < 8 + URLBAR_H {
            B_EDIT = true;
            return;
        }
        B_EDIT = false;
        // page links (HOME only)
        if B_PAGE == B_HOME {
            let ly0 = PAGE_Y + 100;
            if ly >= ly0 && ly < ly0 + 80 {
                let link = (ly - ly0) / 40;
                if lx < 200 {
                    if link == 0 {
                        B_PAGE = B_ABOUT;
                    } else if link == 1 {
                        B_PAGE = B_MANUAL;
                    }
                }
            }
        }
    }
}

fn browser_key(code: u32) {
    unsafe {
        match code {
            256 => {
                // Enter navigates (implicit edit mode on)
                browser_navigate();
                B_EDIT = true;
            }
            257 => {
                if B_URL_LEN > 0 {
                    B_URL_LEN -= 1;
                    B_URL[B_URL_LEN] = 0;
                }
            }
            258 => {
                B_EDIT = false;
                B_URL_LEN = 0;
            }
            c if (32..126).contains(&c) => {
                if B_URL_LEN < 24 {
                    B_URL[B_URL_LEN] = c as u8;
                    B_URL_LEN += 1;
                }
            }
            _ => {}
        }
    }
}

// =====================================================================
// ABOUT
// =====================================================================

fn about_draw(x: i32, y: i32, w: i32, h: i32) {
    gui::fill_rect(x, y, w, h, 0xFF202028);
    // monogram "A" strokes
    gui::draw_line_t(x + 30, y + 78, x + 52, y + 20, 5, 0xFFB9A8F5);
    gui::draw_line_t(x + 52, y + 20, x + 74, y + 78, 5, 0xFFF5A8E8);
    gui::draw_line_t(x + 38, y + 58, x + 66, y + 58, 5, 0xFFFFFFFF);
    gui::draw_text(x + 104, y + 28, "ASPECT OS", 5, 0xFFFFFFFF);
    gui::draw_text(x + 104, y + 76, "VERSION 0.4 - MILESTONE 4", 2, 0xFF9A9AA6);
    gui::fill_rect(x + 24, y + 120, w - 48, 1, 0xFF33333E);
    draw_multiline(x + 24, y + 136,
        "WINDOW COMPOSITOR WITH DRAG,\nFOCUS AND TRAFFIC LIGHTS.\n\nFILE MANAGER, PAINT, TERMINAL,\nCALCULATOR, SETTINGS, BROWSER.\n\n100% OWN RUST CODE.", 2, 0xFFC0C0CA, w - 48);
}

// =====================================================================
// dispatch
// =====================================================================

pub fn draw(app: AppId, x: i32, y: i32, w: i32, h: i32) {
    match app {
        AppId::Files => files_draw(x, y, w, h),
        AppId::Terminal => terminal_draw(x, y, w, h),
        AppId::Calc => calc_draw(x, y, w, h),
        AppId::Paint => paint_draw(x, y, w, h),
        AppId::Settings => settings_draw(x, y, w, h),
        AppId::Browser => browser_draw(x, y, w, h),
        AppId::About => about_draw(x, y, w, h),
    }
}

pub fn on_click(app: AppId, lx: i32, ly: i32) {
    match app {
        AppId::Files => files_click(lx, ly),
        AppId::Calc => calc_click(lx, ly),
        AppId::Paint => paint_click(lx, ly),
        AppId::Settings => settings_click(lx, ly),
        AppId::Browser => browser_click(lx, ly),
        _ => {}
    }
}

pub fn on_move(app: AppId, lx: i32, ly: i32) {
    if let AppId::Paint = app {
        paint_move(lx, ly);
    }
}

pub fn on_key(app: AppId, code: u32) {
    match app {
        AppId::Terminal => terminal_key(code),
        AppId::Calc => calc_key(code),
        AppId::Browser => browser_key(code),
        AppId::Files => files_key(code),
        _ => {}
    }
}
