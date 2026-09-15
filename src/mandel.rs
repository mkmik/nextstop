//! Mandelbrot — the NeXTSTEP 1.0 demo: escape-time set in four grays with selectable dithering.
use crate::app::*;
use crate::chrome::*;
use crate::geom::{rect, Pt, Rect};
use crate::paint::*;
use std::rc::Rc;
use std::time::Instant;

pub const IMG_W: i32 = 480;
pub const IMG_H: i32 = 320;
pub const WIN_W: i32 = 520;
pub const WIN_H: i32 = 528;
const MODES: [&str; 4] = ["Standard PS", "Knight's Tour", "Ohlfs Mix", "Error Diffusion"];
const GRAYS: [u8; 4] = [0, 85, 170, 255];

pub struct Mandel {
    pub cx: f64, pub cy: f64, pub scale: f64, // view centre and width in the complex plane
    pub depth: u32,
    pub mode: usize,
    pub iters: Option<Vec<u16>>,
    pub img: Option<Rc<Image>>,
    pub seq: u64,
    pub busy: bool,
    pub ms: u32,
    pub band: Option<Rect>,
    knight: [u8; 64],
}
impl Default for Mandel {
    fn default() -> Self { Mandel { cx: -0.5, cy: 0.0, scale: 3.0, depth: 128, mode: 0, iters: None, img: None, seq: 0, busy: false, ms: 0, band: None, knight: knights_tour() } }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MBtn { Radio(usize), DepthUp, DepthDown, Reset, Save }

/// Threshold matrix from a knight's tour of the 8×8 board (Warnsdorff's rule; Bayer-like fallback).
fn knights_tour() -> [u8; 64] {
    let moves: [(i32, i32); 8] = [(1, 2), (2, 1), (2, -1), (1, -2), (-1, -2), (-2, -1), (-2, 1), (-1, 2)];
    let degree = |v: &[bool; 64], x: i32, y: i32| moves.iter().filter(|(dx, dy)| { let (nx, ny) = (x + dx, y + dy); (0..8).contains(&nx) && (0..8).contains(&ny) && !v[(ny * 8 + nx) as usize] }).count();
    for start in 0..64 {
        let mut visited = [false; 64];
        let mut order = [0u8; 64];
        let (mut x, mut y) = (start % 8, start / 8);
        visited[start as usize] = true;
        order[start as usize] = 0;
        let mut ok = true;
        for step in 1..64u8 {
            let mut best: Option<(usize, i32, i32)> = None;
            for (dx, dy) in moves {
                let (nx, ny) = (x + dx, y + dy);
                if !(0..8).contains(&nx) || !(0..8).contains(&ny) || visited[(ny * 8 + nx) as usize] { continue; }
                let d = degree(&visited, nx, ny);
                if best.is_none_or(|(bd, _, _)| d < bd) { best = Some((d, nx, ny)); }
            }
            match best { Some((_, nx, ny)) => { x = nx; y = ny; visited[(y * 8 + x) as usize] = true; order[(y * 8 + x) as usize] = step; } None => { ok = false; break; } }
        }
        if ok { return order; }
    }
    let mut m = [0u8; 64];
    for (i, v) in m.iter_mut().enumerate() { *v = ((i % 8) as u8 * 8 + (i / 8) as u8) % 64; }
    m
}

fn compute(cx: f64, cy: f64, scale: f64, depth: u32, w: i32, h: i32) -> Vec<u16> {
    let mut out = vec![0u16; (w * h) as usize];
    let step = scale / w as f64;
    for py in 0..h {
        let y0 = cy - (py as f64 - h as f64 / 2.0) * step;
        for px in 0..w {
            let x0 = cx + (px as f64 - w as f64 / 2.0) * step;
            let (mut x, mut y, mut n) = (0.0f64, 0.0f64, 0u32);
            while x * x + y * y <= 4.0 && n < depth { let xt = x * x - y * y + x0; y = 2.0 * x * y + y0; x = xt; n += 1; }
            out[(py * w + px) as usize] = n as u16;
        }
    }
    out
}

/// Four-level dithering of the gray image. Returns a level index (0..3) per pixel.
fn dither(gray: &[f32], w: i32, h: i32, mode: usize, knight: &[u8; 64]) -> Vec<u8> {
    const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    let n = (w * h) as usize;
    let mut out = vec![0u8; n];
    if mode == 3 { // Floyd–Steinberg, serpentine
        let mut buf: Vec<f32> = gray.to_vec();
        for y in 0..h {
            let ltr = y % 2 == 0;
            for i in 0..w {
                let x = if ltr { i } else { w - 1 - i };
                let idx = (y * w + x) as usize;
                let v = buf[idx].clamp(0.0, 255.0);
                let lvl = (v / 85.0).round().clamp(0.0, 3.0);
                out[idx] = lvl as u8;
                let err = v - lvl * 85.0;
                let dir = if ltr { 1 } else { -1 };
                let mut spread = |dx: i32, dy: i32, k: f32| { let (nx, ny) = (x + dx * dir, y + dy); if (0..w).contains(&nx) && ny < h { buf[(ny * w + nx) as usize] += err * k; } };
                spread(1, 0, 7.0 / 16.0); spread(-1, 1, 3.0 / 16.0); spread(0, 1, 5.0 / 16.0); spread(1, 1, 1.0 / 16.0);
            }
        }
        return out;
    }
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            let t = match mode {
                0 => (BAYER[(y % 4) as usize][(x % 4) as usize] as f32 + 0.5) / 16.0,
                1 => (knight[((y % 8) * 8 + x % 8) as usize] as f32 + 0.5) / 64.0,
                _ => { // "Ohlfs Mix": ordered pattern blended with hashed noise
                    let mut hsh = (x as u32).wrapping_mul(0x9E37_79B9) ^ (y as u32).wrapping_mul(0x85EB_CA6B);
                    hsh ^= hsh >> 15; hsh = hsh.wrapping_mul(0x2C1B_3C6D); hsh ^= hsh >> 12;
                    let noise = (hsh & 0xFFFF) as f32 / 65536.0;
                    (BAYER[(y % 4) as usize][(x % 4) as usize] as f32 / 16.0 + noise) / 2.0
                }
            };
            let q = gray[idx].clamp(0.0, 255.0) / 85.0;
            let base = q.floor();
            let lvl = (base + if q - base > t { 1.0 } else { 0.0 }).min(3.0);
            out[idx] = lvl as u8;
        }
    }
    out
}

fn radio(p: &mut Painter, x: i32, y: i32, on: bool) {
    // 13 px circle, sunken (dark top-left, white bottom-right), black dot when selected
    let r = 6i32;
    for dy in -r..=r {
        let hw = ((r * r - dy * dy) as f32).sqrt().round() as i32;
        p.fill(rect(x + r - hw, y + r + dy, 2 * hw + 1, 1), LIGHT);
        for dx in [-hw, hw] { p.fill(rect(x + r + dx, y + r + dy, 1, 1), if dx + dy < 0 { DARK } else if dx + dy > 0 { WHITE } else { DARK }); }
    }
    if on { for dy in -2i32..=2 { let hw = if dy.abs() == 2 { 1 } else { 2 }; p.fill(rect(x + r - hw, y + r + dy, 2 * hw + 1, 1), BLACK); } }
}
fn dashed_outline(p: &mut Painter, r: Rect) {
    for i in 0..r.w { let c = if i % 4 < 2 { BLACK } else { WHITE }; p.fill(rect(r.x + i, r.y, 1, 1), c); p.fill(rect(r.x + i, r.bottom() - 1, 1, 1), c); }
    for i in 0..r.h { let c = if i % 4 < 2 { BLACK } else { WHITE }; p.fill(rect(r.x, r.y + i, 1, 1), c); p.fill(rect(r.right() - 1, r.y + i, 1, 1), c); }
}

impl App {
    fn m_image(&self) -> Rect { let c = self.content_rect(WinKind::Mandelbrot); rect(c.x + 20, c.y + 12, IMG_W, IMG_H) }
    fn m_radio(&self, i: usize) -> Rect { let c = self.content_rect(WinKind::Mandelbrot); rect(c.x + 30, c.y + 396 + i as i32 * 24, 130, 16) }
    fn m_field(&self, i: usize) -> Rect { let c = self.content_rect(WinKind::Mandelbrot); rect(c.x + 256, c.y + 382 + i as i32 * 22, 118, 20) }
    fn m_depth_btn(&self, up: bool) -> Rect { let f = self.m_field(3); rect(f.right() + 6 + if up { 0 } else { 20 }, f.y, 18, 19) }
    fn m_reset(&self) -> Rect { let c = self.content_rect(WinKind::Mandelbrot); rect(c.x + 430, c.y + 382, 70, 70) }
    fn m_save(&self) -> Rect { let c = self.content_rect(WinKind::Mandelbrot); rect(c.x + 430, c.y + 462, 70, BTN_H - 1) }

    /// Start (or restart) the computation for the current view.
    pub fn mandel_run(&mut self) {
        let m = &mut self.mandel;
        m.seq += 1; m.busy = true;
        let (seq, cx, cy, scale, depth) = (m.seq, m.cx, m.cy, m.scale, m.depth);
        self.spawn(move || {
            let t = Instant::now();
            let iters = compute(cx, cy, scale, depth, IMG_W, IMG_H);
            Job::Mandel { seq, iters, ms: t.elapsed().as_millis() as u32 }
        });
    }
    pub fn mandel_ensure(&mut self) { if self.mandel.iters.is_none() && !self.mandel.busy { self.mandel_run(); } }
    pub fn mandel_done(&mut self, seq: u64, iters: Vec<u16>, ms: u32) {
        if seq != self.mandel.seq { return; }
        self.mandel.busy = false; self.mandel.ms = ms; self.mandel.iters = Some(iters);
        self.mandel_redither();
    }
    fn mandel_redither(&mut self) {
        let m = &mut self.mandel;
        let Some(iters) = &m.iters else { return };
        let depth = m.depth as f32;
        let gray: Vec<f32> = iters.iter().map(|&n| if n as u32 >= m.depth { 0.0 } else { 255.0 * (1.0 - (n as f32 / depth).sqrt()) }).collect();
        let levels = dither(&gray, IMG_W, IMG_H, m.mode, &m.knight);
        let px = levels.iter().map(|&l| { let v = GRAYS[l as usize]; [v, v, v, 255] }).collect();
        m.img = Some(Rc::new(Image { w: IMG_W as u32, h: IMG_H as u32, px }));
    }
    fn mandel_zoom(&mut self, center: Pt, factor: f64) {
        let img = self.m_image();
        let m = &mut self.mandel;
        let step = m.scale / IMG_W as f64;
        m.cx += (center.x - img.x - IMG_W / 2) as f64 * step;
        m.cy -= (center.y - img.y - IMG_H / 2) as f64 * step;
        m.scale *= factor;
        self.mandel_run();
    }
    pub fn mandel_btn(&mut self, b: MBtn) {
        match b {
            MBtn::Radio(i) => { self.mandel.mode = i; self.mandel_redither(); }
            MBtn::DepthUp => { self.mandel.depth = (self.mandel.depth * 2).min(4096); self.mandel_run(); }
            MBtn::DepthDown => { self.mandel.depth = (self.mandel.depth / 2).max(8); self.mandel_run(); }
            MBtn::Reset => { let k = self.mandel.knight; let mode = self.mandel.mode; self.mandel = Mandel { mode, knight: k, ..Mandel::default() }; self.mandel_run(); }
            MBtn::Save => self.mandel_save(),
        }
    }
    fn mandel_save(&mut self) {
        let Some(img) = &self.mandel.img else { return };
        let px: Vec<u32> = img.px.iter().map(|c| (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32).collect();
        let png = encode_png(img.w, img.h, &px);
        let path = (1..).map(|n| crate::icons::join(&self.home, &format!("Mandelbrot-{n}.png"))).find(|p| !std::path::Path::new(p).exists()).unwrap();
        match std::fs::write(&path, png) { Ok(()) => { self.log(format!("saved {path}")); self.fv_refresh(); } Err(e) => self.error("Cannot save image", e.to_string()) }
    }

    pub fn mandel_btn_hit(&self, p: Pt) -> Option<Btn> {
        if let Some(i) = (0..4).find(|&i| self.m_radio(i).contains(p)) { return Some(Btn::Mandel(MBtn::Radio(i))); }
        if self.m_depth_btn(true).contains(p) { return Some(Btn::Mandel(MBtn::DepthUp)); }
        if self.m_depth_btn(false).contains(p) { return Some(Btn::Mandel(MBtn::DepthDown)); }
        if self.m_reset().contains(p) { return Some(Btn::Mandel(MBtn::Reset)); }
        if self.m_save().contains(p) { return Some(Btn::Mandel(MBtn::Save)); }
        None
    }
    pub fn mandel_mouse_down(&mut self, p: Pt) {
        if let Some(b) = self.mandel_btn_hit(p) { self.capture = Some(Capture::Press(b)); return; }
        if self.m_image().contains(p) && !self.mandel.busy { self.capture = Some(Capture::MandelDrag { start: p }); }
    }
    pub fn mandel_drag(&mut self, start: Pt, p: Pt) {
        let img = self.m_image();
        let x0 = start.x.min(p.x).max(img.x); let y0 = start.y.min(p.y).max(img.y);
        let x1 = start.x.max(p.x).min(img.right()); let y1 = start.y.max(p.y).min(img.bottom());
        self.mandel.band = Some(rect(x0, y0, x1 - x0, y1 - y0));
    }
    pub fn mandel_release(&mut self, start: Pt, p: Pt, mods: Mods) {
        let band = self.mandel.band.take();
        if !self.m_image().contains(start) { return; }
        match band {
            Some(b) if b.w >= 8 && b.h >= 8 => {
                let factor = (b.w as f64 / IMG_W as f64).max(b.h as f64 / IMG_H as f64);
                self.mandel_zoom(b.center(), factor);
            }
            _ => { if self.m_image().contains(p) { self.mandel_zoom(p, if mods.shift { 2.0 } else { 0.5 }); } }
        }
    }

    pub fn mandel_draw(&self, p: &mut Painter, c: Rect) {
        let pressed = self.pressed();
        let img = self.m_image();
        let frame = img.inset(-2);
        p.fill(frame, WHITE);
        p.fill(rect(frame.x, frame.y, frame.w, 1), DARK); p.fill(rect(frame.x, frame.y + 1, frame.w, 1), BLACK);
        p.fill(rect(frame.x, frame.y, 1, frame.h), DARK); p.fill(rect(frame.x + 1, frame.y, 1, frame.h), BLACK);
        p.hline(frame.x, frame.bottom() - 1, frame.w, WHITE); p.vline(frame.right() - 1, frame.y, frame.h, WHITE);
        match &self.mandel.img {
            Some(im) => p.image(im, img.x, img.y, Some((IMG_W, IMG_H)), 255),
            None => p.text_in(FontId::Regular, 12, img, Align::Center, "Computing…", DARK),
        }
        if self.mandel.busy && self.mandel.img.is_some() { p.text(FontId::Regular, 12, img.x + 6, img.y + 16, "Computing…", BLACK); }
        if let Some(b) = self.mandel.band { if b.w > 1 && b.h > 1 { dashed_outline(p, b); } }
        let m = &self.mandel;
        p.text(FontId::Regular, 12, c.x + 20, c.y + 361, "Elapsed time:", BLACK);
        field(p, rect(c.x + 110, c.y + 348, 80, 20), &format!("{:.3} s", m.ms as f32 / 1000.0));
        // Dithering group
        let g = rect(c.x + 20, c.y + 386, 150, 108);
        p.hline(g.x, g.y, g.w, DARK); p.vline(g.x, g.y, g.h, DARK); p.hline(g.x, g.bottom() - 1, g.w, WHITE); p.vline(g.right() - 1, g.y, g.h, WHITE);
        let tw = p.text_width(FontId::Regular, 12, "Dithering");
        p.fill(rect(g.x + 8, g.y - 6, tw + 8, 12), LIGHT);
        p.text(FontId::Regular, 12, g.x + 12, g.y + 4, "Dithering", BLACK);
        for (i, name) in MODES.iter().enumerate() {
            let r = self.m_radio(i);
            radio(p, r.x, r.y + 1, m.mode == i);
            p.text_in(FontId::Regular, 12, rect(r.x + 20, r.y, r.w - 20, r.h), Align::Left, name, BLACK);
        }
        let values = [format!("{:.6}", m.cx), format!("{:.6}", m.cy), format!("{:.6}", m.scale), m.depth.to_string(), "4".to_string()];
        for (i, (label, v)) in ["X:", "Y:", "Scale:", "Depth:", "Colors:"].iter().zip(values.iter()).enumerate() {
            let f = self.m_field(i);
            let lw = p.text_width(FontId::Regular, 12, label);
            p.text(FontId::Regular, 12, f.x - 6 - lw, f.y + 14, label, BLACK);
            field(p, f, v);
        }
        for up in [true, false] {
            let r = self.m_depth_btn(up);
            if pressed == Some(Btn::Mandel(if up { MBtn::DepthUp } else { MBtn::DepthDown })) { p.pressed(r) } else { p.raised(r) }
            if up { tri_up(p, r.x + 5, r.y + 6, 7, BLACK) } else { tri_down(p, r.x + 5, r.y + 7, 7, BLACK) }
        }
        let rb = self.m_reset();
        if pressed == Some(Btn::Mandel(MBtn::Reset)) { p.pressed(rb) } else { p.raised(rb) }
        p.text_in(FontId::Bold, 14, rb, Align::Center, "Reset", BLACK);
        button(p, self.m_save(), "Save", pressed == Some(Btn::Mandel(MBtn::Save)), false);
    }
}
