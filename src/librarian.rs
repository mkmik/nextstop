//! Digital Librarian (1990): full-text search over the "bookshelves" on the Shelf.
use crate::app::*;
use crate::backend::{apps, fs};
use crate::chrome::*;
use crate::fileviewer::SHELF_CELL;
use crate::geom::{rect, Pt, Rect};
use crate::icons;
use crate::paint::*;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub const ROW_H: i32 = 16;
const SHELF_H: i32 = 84;      // bookshelf icons with their labels, as on the File Viewer Shelf
const QUERY_H: i32 = 46;      // "Find:" field row under the shelf groove
const NAME_W: i32 = 150;
const MAX_HITS: usize = 300;
const MAX_FILES: usize = 4000;
const MAX_BYTES: usize = 1 << 20;

#[derive(Debug)]
pub struct Hit { pub path: String, pub name: String, pub count: u32, pub line: String }

#[derive(Default)]
pub struct Librarian {
    pub query: String,
    pub books: Vec<String>, // the bookshelves to search (Shelf paths)
    pub hits: Vec<Hit>,
    pub sel: Option<usize>,
    pub scroll: i32,
    pub seq: u64,
    pub busy: bool,
    pub status: String,
    last_click: Option<(usize, Instant)>,
}

/// Rank the text documents under `roots` that contain `needle` (case-insensitive), most hits first.
/// Returns the hits and how many documents were read.
// ponytail: no index — this greps on a background thread, capped at MAX_FILES/MAX_HITS. The 1990
// original needed `ixbuild` because the hardware was a 25 MHz 68030; build one if a bookshelf drags.
pub fn search(roots: Vec<String>, needle: &str) -> (Vec<Hit>, usize) {
    let needle = needle.trim().to_lowercase();
    if needle.is_empty() { return (vec![], 0); }
    let mut hits: Vec<Hit> = vec![];
    let mut scanned = 0usize;
    let mut queue: Vec<PathBuf> = roots.iter().filter_map(|r| fs::check(r).ok()).collect();
    while let Some(dir) = queue.pop() {
        if scanned >= MAX_FILES || hits.len() >= MAX_HITS { break; }
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            if scanned >= MAX_FILES || hits.len() >= MAX_HITS { break; }
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') { continue; }
            // file_type() does not follow symlinks, so a link loop cannot trap the walk
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() { queue.push(e.path()); continue; }
            if !ft.is_file() || !icons::is_text_like(&name) { continue; }
            scanned += 1;
            let path = e.path().to_string_lossy().into_owned();
            let Ok(text) = fs::read_text_head(path.clone(), MAX_BYTES) else { continue };
            let count = text.to_lowercase().matches(&needle).count() as u32;
            if count == 0 { continue; }
            let line = text.lines().find(|l| l.to_lowercase().contains(&needle)).unwrap_or_default();
            hits.push(Hit { path, name, count, line: line.trim().replace('\t', " ").chars().take(160).collect() });
        }
    }
    hits.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    (hits, scanned)
}

impl App {
    fn lib_shelf(&self) -> Rect { let c = self.content_rect(WinKind::Librarian); rect(c.x, c.y, c.w, SHELF_H) }
    fn lib_book_rect(&self, i: usize) -> Rect { let s = self.lib_shelf(); rect(s.x + 8 + i as i32 * SHELF_CELL, s.y + 4, SHELF_CELL, 72) }
    fn lib_field(&self) -> Rect { let c = self.content_rect(WinKind::Librarian); rect(c.x + 48, c.y + SHELF_H + 10, (c.w - 48 - 104).max(40), 21) }
    fn lib_button(&self) -> Rect { let c = self.content_rect(WinKind::Librarian); rect(c.right() - 88, c.y + SHELF_H + 9, 80, BTN_H - 1) }
    fn lib_box(&self) -> Rect {
        let c = self.content_rect(WinKind::Librarian);
        let y = c.y + SHELF_H + QUERY_H;
        rect(c.x + 8, y, c.w - 16, (c.bottom() - y - 8).max(24))
    }
    fn lib_list(&self) -> Rect { let b = self.lib_box(); rect(b.x + SCROLL_W, b.y, b.w - SCROLL_W, b.h) }
    pub fn lib_scroller(&self) -> Scroller {
        let b = self.lib_box();
        let total = self.lib.hits.len() as i32 * ROW_H + 2;
        Scroller::framed(rect(b.x, b.y, SCROLL_W, b.h), true, total, b.h, self.lib.scroll.clamp(0, (total - b.h).max(0)))
    }
    /// Opening the window puts the home folder on the bookshelf, so Search never means "the whole disk".
    pub fn lib_show(&mut self) {
        if !self.lib.books.is_empty() { return; }
        let home = self.home.clone();
        if let Some(b) = self.state.shelf.iter().find(|p| **p == home).or_else(|| self.state.shelf.first()) { self.lib.books = vec![b.clone()]; }
    }
    pub fn lib_search(&mut self) {
        let q = self.lib.query.trim().to_string();
        if q.is_empty() { self.lib.status = "Type a word to look for.".into(); return; }
        let roots = self.lib.books.clone();
        if roots.is_empty() { self.lib.status = "Click a bookshelf to search in.".into(); return; }
        self.lib.seq += 1;
        self.lib.busy = true;
        self.lib.hits.clear();
        self.lib.sel = None;
        self.lib.scroll = 0;
        self.lib.status = format!("Searching {}…", roots.iter().map(|r| icons::basename(r)).collect::<Vec<_>>().join(", "));
        let seq = self.lib.seq;
        self.spawn(move || { let (hits, scanned) = search(roots, &q); Job::Found { seq, hits, scanned } });
    }
    pub fn lib_found(&mut self, seq: u64, hits: Vec<Hit>, scanned: usize) {
        if seq != self.lib.seq { return; }
        self.lib.busy = false;
        self.lib.status = match hits.len() {
            0 => format!("Nothing found in {scanned} documents."),
            n => format!("{n} of {scanned} documents contain “{}”.", self.lib.query.trim()),
        };
        self.lib.hits = hits;
    }
    pub fn lib_open(&mut self, i: usize) {
        let Some(path) = self.lib.hits.get(i).map(|h| h.path.clone()) else { return };
        self.log(format!("open {path}"));
        if let Err(e) = apps::open_path(&path) { self.error("Cannot open", e); }
    }
    fn lib_row_at(&self, p: Pt) -> Option<usize> {
        let list = self.lib_list();
        if !list.contains(p) { return None; }
        let i = ((p.y - list.y - 1 + self.lib_scroller().pos) / ROW_H).max(0) as usize;
        (i < self.lib.hits.len()).then_some(i)
    }
    fn lib_reveal(&mut self, i: usize) {
        let h = self.lib_list().h - 2;
        let pos = self.lib.scroll.min(i as i32 * ROW_H).max((i as i32 + 1) * ROW_H - h);
        self.set_scroll(ScrollId::Librarian, pos);
    }

    pub fn lib_btn_hit(&self, p: Pt) -> Option<Btn> {
        if self.lib_button().contains(p) { return Some(Btn::LibSearch); }
        if let Some(i) = (0..self.state.shelf.len().min(16)).find(|&i| self.lib_book_rect(i).contains(p)) { return Some(Btn::LibBook(i)); }
        match self.lib_scroller().hit(p)? {
            ScrollHit::ArrowA => Some(Btn::ScrollArrow(ScrollId::Librarian, -1)),
            ScrollHit::ArrowB => Some(Btn::ScrollArrow(ScrollId::Librarian, 1)),
            _ => None,
        }
    }
    pub fn lib_mouse_down(&mut self, p: Pt) {
        if let Some(i) = self.lib_row_at(p) {
            let now = self.now;
            let dbl = self.lib.last_click.is_some_and(|(j, t)| j == i && now.duration_since(t) < Duration::from_millis(400));
            self.lib.last_click = if dbl { None } else { Some((i, now)) };
            self.lib.sel = Some(i);
            if dbl { self.lib_open(i); }
            return;
        }
        let sc = self.lib_scroller();
        if self.scroller_down(ScrollId::Librarian, sc, p) { return; }
        if let Some(b) = self.lib_btn_hit(p) { self.capture = Some(Capture::Press(b)); }
    }
    /// Clicking a bookshelf narrows the search to it (click again to deselect).
    pub fn lib_toggle_book(&mut self, i: usize) {
        let Some(path) = self.state.shelf.get(i).cloned() else { return };
        match self.lib.books.iter().position(|b| *b == path) {
            Some(j) => { self.lib.books.remove(j); }
            None => self.lib.books.push(path),
        }
    }
    pub fn lib_key(&mut self, k: Key) {
        let n = self.lib.hits.len();
        match k {
            Key::Char(c) => self.lib.query.push(c),
            Key::Backspace | Key::Delete => { self.lib.query.pop(); }
            Key::Enter => self.lib_search(),
            Key::Escape => self.lib.query.clear(),
            Key::Down | Key::Up if n > 0 => {
                let i = match (k, self.lib.sel) {
                    (Key::Down, Some(c)) => (c + 1).min(n - 1),
                    (Key::Up, Some(c)) => c.saturating_sub(1),
                    _ => 0,
                };
                self.lib.sel = Some(i);
                self.lib_reveal(i);
            }
            _ => {}
        }
    }

    pub fn lib_draw(&self, p: &mut Painter, c: Rect) {
        let pressed = self.pressed();
        // Bookshelves: the File Viewer Shelf, with the ones being searched on a white cell
        if self.state.shelf.is_empty() {
            p.text(FontId::Regular, 12, c.x + 10, c.y + 30, "No bookshelves yet — drag folders onto the File Viewer Shelf.", DARK);
        }
        for (i, path) in self.state.shelf.iter().take(16).enumerate() {
            let r = self.lib_book_rect(i);
            if self.lib.books.contains(path) { p.fill(rect(r.x + 14, r.y, 68, 70), WHITE); }
            p.icon(self.fv_path_icon(path), r.x + 24, r.y + 4, 48);
            let t = p.ellipsize_mid(FontId::Regular, 12, &icons::basename(path), r.w - 6);
            p.text_in(FontId::Regular, 12, rect(r.x + 3, r.y + 54, r.w - 6, 14), Align::Center, &t, BLACK);
        }
        let sh = self.lib_shelf();
        p.hline(sh.x, sh.bottom(), sh.w, DARK);
        p.hline(sh.x, sh.bottom() + 1, sh.w, WHITE);
        // Query row
        let f = self.lib_field();
        let base = p.baseline(FontId::Bold, 12, f.y, f.h);
        p.text(FontId::Bold, 12, c.x + 8, base, "Find:", BLACK);
        field(p, f, &self.lib.query);
        if self.key == Some(WinKind::Librarian) {
            let w = p.text_width(FontId::Regular, 12, &self.lib.query).min(f.w - 10);
            p.fill(rect(f.x + 5 + w, f.y + 4, 1, f.h - 8), BLACK);
        }
        button(p, self.lib_button(), if self.lib.busy { "Searching" } else { "Search" }, pressed == Some(Btn::LibSearch), true);
        let st = rect(c.x + 10, f.bottom() + 1, c.w - 20, 14);
        p.text_in(FontId::Regular, 10, st, Align::Left, &p.ellipsize(FontId::Regular, 10, &self.lib.status, st.w), DARK);
        // Results: name, first matching line, hit count
        let (b, list, sc) = (self.lib_box(), self.lib_list(), self.lib_scroller());
        p.sunken(b);
        p.push_clip(rect(list.x, list.y + 1, list.w - 1, list.h - 2));
        let first = (sc.pos / ROW_H).max(0) as usize;
        for (i, h) in self.lib.hits.iter().enumerate().skip(first).take((list.h / ROW_H + 2) as usize) {
            let r = rect(list.x, list.y + 1 + i as i32 * ROW_H - sc.pos, list.w - 1, ROW_H);
            if self.lib.sel == Some(i) { p.fill(r, WHITE); }
            let name = p.ellipsize_mid(FontId::Bold, 12, &h.name, NAME_W);
            p.text_in(FontId::Bold, 12, rect(r.x + 5, r.y, NAME_W, r.h), Align::Left, &name, BLACK);
            let ct = h.count.to_string();
            let cw = p.text_width(FontId::Regular, 11, &ct);
            p.text_in(FontId::Regular, 11, rect(r.right() - cw - 6, r.y, cw, r.h), Align::Left, &ct, DARK);
            let ex = rect(r.x + 5 + NAME_W + 8, r.y, r.right() - cw - 14 - (r.x + 5 + NAME_W + 8), r.h);
            if ex.w > 24 { p.text_in(FontId::Regular, 11, ex, Align::Left, &p.ellipsize(FontId::Regular, 11, &h.line, ex.w), DARK); }
        }
        p.pop_clip();
        let pr = match pressed { Some(Btn::ScrollArrow(ScrollId::Librarian, d)) => Some(if d < 0 { ScrollHit::ArrowA } else { ScrollHit::ArrowB }), _ => None };
        sc.draw(p, pr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_by_hit_count_skips_hidden_and_binary_and_reports_the_line() {
        let d = std::env::temp_dir().join(format!("reworkspace-lib-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("sub")).unwrap();
        std::fs::create_dir_all(d.join(".git")).unwrap();
        std::fs::write(d.join("one.txt"), "nothing here\nthe Ship sails\n").unwrap();
        std::fs::write(d.join("sub/two.md"), "ship ship\nSHIP\n").unwrap();
        std::fs::write(d.join("three.txt"), "no match at all").unwrap();
        std::fs::write(d.join("photo.png"), "ship ship ship ship").unwrap();
        std::fs::write(d.join(".git/hidden.txt"), "ship ship ship ship ship").unwrap();
        let root = vec![d.to_string_lossy().into_owned()];
        let (hits, scanned) = search(root.clone(), "SHIP");
        assert_eq!(hits.iter().map(|h| (h.name.as_str(), h.count)).collect::<Vec<_>>(), [("two.md", 3), ("one.txt", 1)]);
        assert_eq!(hits[1].line, "the Ship sails");
        assert_eq!(scanned, 3); // one.txt, two.md, three.txt — not the PNG, not the dot folder
        assert!(search(root, "  ").0.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }
}
