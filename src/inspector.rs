//! Inspector panel (§7.9).
use crate::app::*;
use crate::backend::fs::{self, Entry, Meta};
use crate::chrome::*;
use crate::geom::{rect, Pt, Rect};
use crate::icons;
use crate::paint::*;
use std::rc::Rc;

const MODES: [&str; 4] = ["Attributes", "Contents", "Tools", "Access Control"];

#[derive(Default)]
pub struct Inspector {
    pub mode: usize,
    pub entry: Option<Entry>,
    pub meta: Option<Result<Meta, String>>,
    pub text: Option<Vec<String>>,
    pub image: Option<Rc<Image>>,
    pub msg: Option<String>,
    pub dir_size: Option<u64>,
    pub computing: bool,
    pub popup_open: bool,
    pub scroll: i32,
}

fn fmt_date(s: Option<i64>) -> String {
    let Some(s) = s else { return "—".into() };
    // civil-from-days (Howard Hinnant), UTC
    let days = s.div_euclid(86400); let rem = s.rem_euclid(86400);
    let z = days + 719468; let era = z.div_euclid(146097); let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1; let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", rem / 3600, (rem % 3600) / 60)
}
fn fmt_size(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() { if i > 0 && (s.len() - i) % 3 == 0 { out.push(','); } out.push(c); }
    format!("{out} bytes")
}

impl App {
    fn insp_popup_rect(&self) -> Rect { let c = self.content_rect(WinKind::Inspector); rect(c.x + 8, c.y + 68, c.w - 16, 23) }
    fn insp_body(&self) -> Rect { let c = self.content_rect(WinKind::Inspector); rect(c.x + 8, c.y + 108, c.w - 16, c.h - 116) }
    fn insp_row_rect(&self, i: usize) -> Rect { let b = self.insp_popup_rect(); rect(b.x, b.bottom() + 1 + i as i32 * 20, b.w, 20) }
    fn insp_compute_rect(&self) -> Rect { let b = self.insp_body(); rect(b.x + 80 + 40, b.y + 40, 64, 20) }
    pub fn insp_text_scroller(&self) -> Option<Scroller> {
        let lines = self.insp.text.as_ref()?.len() as i32;
        let b = self.insp_body();
        let total = lines * 13 + 4;
        Some(Scroller::framed(rect(b.x, b.y, SCROLL_W, b.h), true, total, b.h, self.insp.scroll.clamp(0, (total - b.h).max(0))))
    }

    pub fn insp_update(&mut self, e: Option<Entry>) {
        let same = match (&e, &self.insp.entry) { (Some(a), Some(b)) => a.path == b.path && a.modified == b.modified, (None, None) => true, _ => false };
        if same { return; }
        self.insp.entry = e;
        self.insp_refresh();
    }
    /// (Re)load what the current mode shows for the current entry.
    pub fn insp_refresh(&mut self) {
        let i = &mut self.insp;
        i.meta = None; i.text = None; i.image = None; i.msg = None; i.dir_size = None; i.computing = false; i.scroll = 0;
        if !self.win(WinKind::Inspector).visible { return; }
        let Some(e) = self.insp.entry.clone() else { return };
        if self.insp.mode == 0 {
            let path = e.path.clone();
            self.spawn(move || Job::Meta { result: fs::file_meta(path.clone()), path });
        } else {
            if e.is_dir { self.insp.msg = Some("No contents viewer for folders".into()); return; }
            let path = e.path.clone();
            if icons::image_mime(&e.name) {
                self.spawn(move || Job::ImageBytes { result: fs::read_file_b64_bytes(&path, 32 * 1024 * 1024), path });
            } else if icons::is_text_like(&e.name) && e.size < 256 * 1024 {
                self.spawn(move || Job::Text { result: fs::read_text_head(path.clone(), 256 * 1024), path });
            } else { self.insp.msg = Some("No contents viewer for this type".into()); }
        }
    }
    fn insp_is_current(&self, path: &str) -> bool { self.insp.entry.as_ref().is_some_and(|e| e.path == path) }
    pub fn insp_meta(&mut self, path: String, result: Result<Meta, String>) { if self.insp_is_current(&path) { self.insp.meta = Some(result); } }
    pub fn insp_text(&mut self, path: String, result: Result<String, String>) {
        if !self.insp_is_current(&path) { return; }
        match result { Ok(t) => self.insp.text = Some(t.lines().take(200).map(|l| l.replace('\t', "    ")).collect()), Err(e) => self.insp.msg = Some(e) }
    }
    pub fn insp_image(&mut self, path: String, result: Result<Vec<u8>, String>) {
        if !self.insp_is_current(&path) { return; }
        match result.and_then(|b| decode_png(&b)) { Ok(img) => self.insp.image = Some(Rc::new(img)), Err(e) => self.insp.msg = Some(e) }
    }
    pub fn insp_dir_size(&mut self, path: String, result: Result<u64, String>) {
        if !self.insp_is_current(&path) { return; }
        self.insp.computing = false;
        match result { Ok(n) => self.insp.dir_size = Some(n), Err(e) => self.insp.msg = Some(e) }
    }
    pub fn insp_compute(&mut self) {
        let Some(e) = self.insp.entry.clone() else { return };
        self.insp.computing = true;
        self.spawn(move || Job::DirSize { result: fs::dir_size(e.path.clone()), path: e.path });
    }
    pub fn insp_set_mode(&mut self, i: usize) {
        self.insp.popup_open = false;
        if i >= 2 { return; }
        self.insp.mode = i;
        self.insp_refresh();
    }
    pub fn insp_btn_hit(&self, p: Pt) -> Option<Btn> {
        if self.insp.popup_open { return (0..MODES.len()).find(|&i| self.insp_row_rect(i).contains(p)).filter(|&i| i < 2).map(Btn::InspRow); }
        if self.insp_popup_rect().contains(p) { return Some(Btn::InspPopup); }
        if self.insp.mode == 0 && self.insp.entry.as_ref().is_some_and(|e| e.is_dir) && self.insp.dir_size.is_none() && !self.insp.computing && self.insp_compute_rect().contains(p) { return Some(Btn::InspCompute); }
        if let Some(sc) = self.insp_text_scroller() { return match sc.hit(p)? { ScrollHit::ArrowA => Some(Btn::ScrollArrow(ScrollId::InspText, -1)), ScrollHit::ArrowB => Some(Btn::ScrollArrow(ScrollId::InspText, 1)), _ => None }; }
        None
    }
    pub fn insp_mouse_down(&mut self, p: Pt) {
        if self.insp.popup_open {
            match self.insp_btn_hit(p) { Some(b) => self.capture = Some(Capture::Press(b)), None => self.insp.popup_open = false }
            return;
        }
        if let Some(sc) = self.insp_text_scroller() { if self.scroller_down(ScrollId::InspText, sc, p) { return; } }
        if let Some(b) = self.insp_btn_hit(p) { self.capture = Some(Capture::Press(b)); }
    }

    pub fn insp_draw(&self, p: &mut Painter, c: Rect) {
        let pressed = self.pressed();
        let Some(e) = &self.insp.entry else { p.text(FontId::Regular, 12, c.x + 8, c.y + 24, "No selection", DARK); return };
        p.icon(icons::icon_for(&e.name, e.is_dir, e.is_app), c.x + 8, c.y + 8, 48);
        let name = p.ellipsize_mid(FontId::Bold, 12, &e.name, c.w - 72);
        p.text_in(FontId::Bold, 12, rect(c.x + 64, c.y + 8, c.w - 72, 48), Align::Left, &name, BLACK);
        let b = self.insp_body();
        if self.insp.mode == 0 {
            let mut y = b.y;
            let mut row = |p: &mut Painter, k: &str, v: &str| {
                p.text(FontId::Bold, 12, b.x, y + 12, k, BLACK);
                let v = p.ellipsize(FontId::Regular, 12, v, b.w - 80);
                p.text(FontId::Regular, 12, b.x + 80, y + 12, &v, BLACK);
                y += 20;
            };
            let m = self.insp.meta.as_ref().and_then(|r| r.as_ref().ok());
            row(p, "Path", &e.path);
            row(p, "Kind", &if e.is_symlink { "Symbolic link".to_string() } else { icons::kind_for(&e.name, e.is_dir, e.is_app) });
            if e.is_dir { row(p, "Size", &self.insp.dir_size.map_or_else(|| if self.insp.computing { "…".to_string() } else { "—".to_string() }, fmt_size)); }
            else { row(p, "Size", &fmt_size(m.map_or(e.size, |m| m.size))); }
            row(p, "Modified", &fmt_date(Some(m.map_or(e.modified, |m| m.modified))));
            row(p, "Created", &fmt_date(m.and_then(|m| m.created)));
            row(p, "Permissions", m.map_or("—", |m| m.mode.as_str()));
            if let Some(o) = m.and_then(|m| m.owner.as_deref()) { row(p, "Owner", o); }
            if let Some(Err(err)) = &self.insp.meta { row(p, "Error", err); }
            if e.is_dir && self.insp.dir_size.is_none() && !self.insp.computing { button(p, self.insp_compute_rect(), "Compute", pressed == Some(Btn::InspCompute), false); }
        } else if let Some(lines) = &self.insp.text {
            let sc = self.insp_text_scroller().unwrap();
            let tr = rect(b.x + SCROLL_W, b.y, b.w - SCROLL_W, b.h);
            p.sunken(tr);
            p.push_clip(tr.inset(1));
            let first = (sc.pos / 13).max(0) as usize;
            for (i, l) in lines.iter().enumerate().skip(first).take((b.h / 13 + 2) as usize) {
                p.text(FontId::Mono, 11, tr.x + 4, tr.y + 2 + i as i32 * 13 - sc.pos + 10, l, BLACK);
            }
            p.pop_clip();
            let pr = match pressed { Some(Btn::ScrollArrow(ScrollId::InspText, d)) => Some(if d < 0 { ScrollHit::ArrowA } else { ScrollHit::ArrowB }), _ => None };
            sc.draw(p, pr);
        } else if let Some(img) = &self.insp.image {
            let k = (240.0 / img.w.max(img.h).max(1) as f32).min(1.0 / p.s * 1.0);
            let (w, h) = ((img.w as f32 * k).max(1.0) as i32, (img.h as f32 * k).max(1.0) as i32);
            p.image(img, b.x + (b.w - w) / 2, b.y + 8, Some((w, h)), 255);
        } else if let Some(m) = &self.insp.msg { p.text(FontId::Regular, 12, b.x, b.y + 16, m, DARK); }
        // popup button + list (drawn last so the list overlaps the body)
        let pr = self.insp_popup_rect();
        button(p, pr, "", pressed == Some(Btn::InspPopup), false);
        p.text_in(FontId::Regular, 12, rect(pr.x + 8, pr.y, pr.w - 28, pr.h), Align::Left, MODES[self.insp.mode], BLACK);
        tri_up(p, pr.right() - 14, pr.y + 5, 7, BLACK);
        tri_down(p, pr.right() - 14, pr.y + 13, 7, BLACK);
        if self.insp.popup_open {
            for (i, m) in MODES.iter().enumerate() {
                let r = self.insp_row_rect(i);
                let hi = pressed == Some(Btn::InspRow(i));
                if hi { p.fill(r, BLACK); p.bevel(r, WHITE, DARK); } else { p.raised(r); }
                let fg = if hi { WHITE } else if i >= 2 { DARK } else { BLACK };
                if i == self.insp.mode { p.fill(rect(r.x + 1, r.y + 7, 6, 6), fg); }
                p.text_in(FontId::Regular, 12, rect(r.x + 8, r.y, r.w - 16, r.h), Align::Left, m, fg);
            }
            let all = rect(self.insp_row_rect(0).x - 1, self.insp_row_rect(0).y - 1, pr.w + 2, MODES.len() as i32 * 20 + 2);
            p.outline(all, BLACK);
        }
    }
}
