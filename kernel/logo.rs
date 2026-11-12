// ASPECT OS - wordmark "ASPECT OS": vector strokes in Rust
use crate::gui::{draw_line_t, px_blend};

#[derive(Clone, Copy)]
struct Seg {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

// letters on an 8-tall grid
const SEG_A: [Seg; 3] = [
    Seg { x0: 0, y0: 7, x1: 3, y1: 0 },
    Seg { x0: 3, y0: 0, x1: 6, y1: 7 },
    Seg { x0: 1, y0: 4, x1: 5, y1: 4 },
];
const SEG_S: [Seg; 5] = [
    Seg { x0: 6, y0: 0, x1: 1, y1: 0 },
    Seg { x0: 1, y0: 0, x1: 1, y1: 4 },
    Seg { x0: 1, y0: 4, x1: 6, y1: 4 },
    Seg { x0: 6, y0: 4, x1: 6, y1: 7 },
    Seg { x0: 6, y0: 7, x1: 1, y1: 7 },
];
const SEG_P: [Seg; 4] = [
    Seg { x0: 0, y0: 7, x1: 0, y1: 0 },
    Seg { x0: 0, y0: 0, x1: 5, y1: 0 },
    Seg { x0: 5, y0: 0, x1: 5, y1: 3 },
    Seg { x0: 5, y0: 3, x1: 0, y1: 3 },
];
const SEG_E: [Seg; 4] = [
    Seg { x0: 6, y0: 0, x1: 0, y1: 0 },
    Seg { x0: 0, y0: 0, x1: 0, y1: 7 },
    Seg { x0: 0, y0: 7, x1: 6, y1: 7 },
    Seg { x0: 0, y0: 4, x1: 4, y1: 4 },
];
const SEG_C: [Seg; 5] = [
    Seg { x0: 6, y0: 0, x1: 1, y1: 0 },
    Seg { x0: 1, y0: 0, x1: 0, y1: 1 },
    Seg { x0: 0, y0: 1, x1: 0, y1: 6 },
    Seg { x0: 0, y0: 6, x1: 1, y1: 7 },
    Seg { x0: 1, y0: 7, x1: 6, y1: 7 },
];
const SEG_T: [Seg; 2] = [
    Seg { x0: 0, y0: 0, x1: 6, y1: 0 },
    Seg { x0: 3, y0: 0, x1: 3, y1: 7 },
];
const SEG_O: [Seg; 8] = [
    Seg { x0: 1, y0: 0, x1: 5, y1: 0 },
    Seg { x0: 5, y0: 0, x1: 6, y1: 1 },
    Seg { x0: 6, y0: 1, x1: 6, y1: 6 },
    Seg { x0: 6, y0: 6, x1: 5, y1: 7 },
    Seg { x0: 5, y0: 7, x1: 1, y1: 7 },
    Seg { x0: 1, y0: 7, x1: 0, y1: 6 },
    Seg { x0: 0, y0: 6, x1: 0, y1: 1 },
    Seg { x0: 0, y0: 1, x1: 1, y1: 0 },
];

// "ASPECT OS": letters with segments, 0 = space
static LETTERS: [&[Seg]; 9] =
    [&SEG_A, &SEG_S, &SEG_P, &SEG_E, &SEG_C, &SEG_T, &[], &SEG_O, &SEG_S];
static WIDTHS: [i32; 9] = [6, 6, 6, 6, 6, 6, 4, 6, 6];

fn draw_seg(s: &Seg, x0: i32, y0: i32, sc: i32, t: i32, c: u32, alpha: i32) {
    let ax = x0 + s.x0 * sc;
    let ay = y0 + s.y0 * sc;
    let bx = x0 + s.x1 * sc;
    let by = y0 + s.y1 * sc;
    if alpha < 255 {
        let dx = bx - ax;
        let dy = by - ay;
        let n = dx.abs().max(dy.abs()).max(1);
        for i in 0..=n {
            let x = ax + (bx - ax) * i / n;
            let y = ay + (by - ay) * i / n;
            for oy in -(t / 2)..=(t / 2) {
                for ox in -(t / 2)..=(t / 2) {
                    px_blend(x + ox, y + oy, c, alpha);
                }
            }
        }
    } else {
        draw_line_t(ax, ay, bx, by, t, c);
    }
}

// cx, cy = wordmark center, sc = scale (px per grid unit)
pub fn draw_wordmark(cx: i32, cy: i32, sc: i32) {
    let mut total = 0;
    for &w in WIDTHS.iter() {
        total += w * sc + 2 * sc;
    }
    total -= 2 * sc;

    // drop shadow
    let off = sc / 2 + 1;
    let mut x = cx - total / 2;
    for (i, segs) in LETTERS.iter().enumerate() {
        if segs.is_empty() {
            x += WIDTHS[i] * sc + 2 * sc;
            continue;
        }
        for s in *segs {
            draw_seg(s, x + off, cy - 4 * sc + off, sc, sc, 0x000000, 80);
        }
        x += WIDTHS[i] * sc + 2 * sc;
    }
    // main strokes
    let mut x = cx - total / 2;
    for (i, segs) in LETTERS.iter().enumerate() {
        if segs.is_empty() {
            x += WIDTHS[i] * sc + 2 * sc;
            continue;
        }
        for s in *segs {
            draw_seg(s, x, cy - 4 * sc, sc, sc, 0xFFFFFF, 255);
        }
        x += WIDTHS[i] * sc + 2 * sc;
    }
}
