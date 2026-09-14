//! File Viewer (§7.8): Shelf, Icon Path, column Browser. Implemented as methods on App.
use crate::app::*;
use crate::backend::fs::{self, Entry};
use crate::chrome::*;
use crate::geom::{rect, Pt, Rect};
use crate::icons;
use crate::paint::*;
use std::path::Path;
use std::time::{Duration, Instant};

pub const COL_W: i32 = 141;
pub const CELL_H: i32 = 18;

pub struct Column {
    pub id: u64,
    pub dir: Option<String>, // None = synthetic "Computer" column (Windows drives)
    pub entries: Vec<Entry>,
    pub sel: Vec<bool>,
    pub anchor: Option<usize>,
    pub seq: u64,
    pub scroll: i32,
    pub unreadable: bool,
}

pub struct Nav { pub chain: Vec<Option<String>>, pub step: usize }

#[derive(Default)]
pub struct FileViewer {
    pub cols: Vec<Column>,
    pub focus: usize,
    pub hscroll: i32,
    pub typeahead: String,
    pub type_until: Option<Instant>,
    pub last_click: Option<(String, Instant)>,
    pub nav: Option<Nav>,
    pub next_id: u64,
}

pub struct FvLayout { pub shelf: Rect, pub path: Rect, pub browser: Rect, pub cols: Rect, pub hscroll: Scroller }

impl App {
    // ---- layout
    pub fn fv_layout(&self) -> FvLayout {
        let c = self.content_rect(WinKind::FileViewer).inset(1);
        let shelf = rect(c.x, c.y, c.w, 72);
        let path = rect(c.x, c.y + 72, c.w, 72);
        let browser = rect(c.x, c.y + 144, c.w, c.h - 144);
        let inner = browser.inset(1);
        let cols = rect(inner.x, inner.y, inner.w, inner.h - 16);
        let total = self.fv.cols.len() as i32 * COL_W;
        let pos = self.fv.hscroll.clamp(0, (total - cols.w).max(0));
        let hscroll = Scroller { r: rect(inner.x, inner.bottom() - 16, inner.w, 16), vertical: false, total, visible: cols.w, pos };
        FvLayout { shelf, path, browser, cols, hscroll }
    }
    fn shelf_item_rect(lay: &FvLayout, i: usize) -> Rect { rect(lay.shelf.x + 2 + i as i32 * 64, lay.shelf.y, 64, 72) }
    fn path_item_rect(lay: &FvLayout, i: usize) -> Rect { rect(lay.path.x + 4 + i as i32 * 72, lay.path.y + 4, 64, 64) }
    pub fn fv_col_rect(&self, lay: &FvLayout, i: usize) -> Rect { rect(lay.cols.x - lay.hscroll.pos + i as i32 * COL_W, lay.cols.y, COL_W, lay.cols.h) }
    /// (list rect, vertical scroller when the column overflows)
    pub fn fv_col_list(&self, lay: &FvLayout, i: usize) -> (Rect, Option<Scroller>) {
        let cr = self.fv_col_rect(lay, i);
        let col = &self.fv.cols[i];
        let total = col.entries.len() as i32 * CELL_H;
        let full = rect(cr.x, cr.y, cr.w - 1, cr.h);
        if total <= full.h { return (full, None); }
        let list = rect(full.x, full.y, full.w - 16, full.h);
        let sc = Scroller { r: rect(full.right() - 16, full.y, 16, full.h), vertical: true, total, visible: full.h, pos: col.scroll.clamp(0, total - full.h) };
        (list, Some(sc))
    }
    pub fn fv_col_scroller(&self, cid: u64) -> Option<Scroller> {
        let i = self.fv.cols.iter().position(|c| c.id == cid)?;
        let lay = self.fv_layout();
        self.fv_col_list(&lay, i).1
    }
    fn cell_rect(list: Rect, scroll: i32, idx: usize) -> Rect { rect(list.x, list.y + idx as i32 * CELL_H - scroll, list.w, CELL_H) }

    // ---- selection queries
    pub fn fv_selection(&self, ci: usize) -> Vec<Entry> {
        self.fv.cols.get(ci).map_or(vec![], |c| c.entries.iter().zip(&c.sel).filter(|(_, s)| **s).map(|(e, _)| e.clone()).collect())
    }
    /// Selection of the deepest column that has one: what the title, Icon Path and Inspector follow.
    pub fn fv_deep_selection(&self) -> Vec<Entry> {
        (0..self.fv.cols.len()).rev().map(|i| self.fv_selection(i)).find(|s| !s.is_empty()).unwrap_or_default()
    }
    pub fn fv_selected_paths(&self) -> Vec<String> { self.fv_deep_selection().into_iter().map(|e| e.path).collect() }
    pub fn fv_sel_entry(&self) -> Option<Entry> { self.fv_deep_selection().into_iter().next() }
    fn fv_last_dir(&self) -> String { self.fv.cols.last().and_then(|c| c.dir.clone()).unwrap_or_else(|| self.roots.first().cloned().unwrap_or("/".into())) }
    pub fn fv_sel_path(&self) -> String { self.fv_sel_entry().map(|e| e.path).unwrap_or_else(|| self.fv_last_dir()) }
    pub fn fv_current_dir(&self) -> String {
        match self.fv_sel_entry() { None => self.fv_last_dir(), Some(e) => if e.is_dir { e.path } else { icons::dirname(&e.path).unwrap_or(e.path) } }
    }
    pub fn fv_path_components(&self) -> Vec<String> { icons::ancestors(&self.fv_sel_path(), false).into_iter().flatten().collect() }
    /// Icon for a bare path (Shelf items are stored as paths only).
    pub fn fv_path_icon(&self, p: &str) -> &'static str {
        if p == self.home { return "home"; }
        if icons::is_root(p) { return "drive"; }
        let n = icons::basename(p);
        if icons::is_app_path(&n) { "application" } else if icons::ext(&n).is_empty() { "folder" } else { icons::file_icon(&n) } // ponytail: extension heuristic, no stat
    }

    // ---- columns
    fn fv_add_column(&mut self, dir: Option<String>) -> usize {
        self.fv.next_id += 1;
        let id = self.fv.next_id;
        let mut col = Column { id, dir, entries: vec![], sel: vec![], anchor: None, seq: 0, scroll: 0, unreadable: false };
        if col.dir.is_none() {
            col.entries = self.roots.iter().map(|r| Entry { name: r.clone(), path: r.clone(), is_dir: true, is_symlink: false, is_app: false, size: 0, modified: 0, hidden: false }).collect();
            col.sel = vec![false; col.entries.len()];
        }
        self.fv.cols.push(col);
        let ci = self.fv.cols.len() - 1;
        if self.fv.cols[ci].dir.is_some() { self.fv_load(ci, false); }
        ci
    }
    fn fv_load(&mut self, ci: usize, quiet: bool) {
        let col = &mut self.fv.cols[ci];
        col.seq += 1;
        let (id, seq) = (col.id, col.seq);
        let Some(dir) = col.dir.clone() else { return };
        let show_hidden = self.state.show_hidden;
        self.spawn(move || Job::Listed { col_id: id, seq, result: fs::read_entries(Path::new(&dir), show_hidden), quiet });
    }
    fn fv_truncate(&mut self, n: usize) {
        self.fv.cols.truncate(n);
        self.fv.focus = self.fv.focus.min(n.saturating_sub(1));
    }
    fn fv_scroll_end(&mut self) { self.fv.hscroll = i32::MAX / 2; } // clamped by the layout
    fn fv_ensure_visible(&mut self, ci: usize, idx: usize) {
        let lay = self.fv_layout();
        let (list, _) = self.fv_col_list(&lay, ci);
        let col = &mut self.fv.cols[ci];
        let top = idx as i32 * CELL_H;
        if top < col.scroll { col.scroll = top; } else if top + CELL_H > col.scroll + list.h { col.scroll = top + CELL_H - list.h; }
    }
    fn fv_changed(&mut self) {
        let dir = self.fv_current_dir();
        self.win_mut(WinKind::FileViewer).title = format!("File Viewer — {dir}");
        self.state.windows.file_viewer.path = self.fv_sel_path();
        self.dirty();
        let e = self.fv_sel_entry();
        self.insp_update(e);
    }
    fn fv_after_select(&mut self, ci: usize) {
        self.fv_truncate(ci + 1);
        self.fv.focus = ci;
        let sel = self.fv_selection(ci);
        if sel.len() == 1 && sel[0].is_dir { self.fv_add_column(Some(sel[0].path.clone())); self.fv_scroll_end(); }
        self.fv_changed();
    }

    /// Select `path`, opening columns from the root down to it (asynchronously, one column at a time).
    pub fn fv_navigate(&mut self, path: &str) {
        let chain = icons::ancestors(path, self.is_win);
        self.fv_truncate(0);
        self.fv.nav = Some(Nav { chain: chain.clone(), step: 0 });
        let ci = self.fv_add_column(chain[0].clone());
        if chain[0].is_none() { if let Some(nav) = self.fv.nav.take() { self.fv_nav_step(nav, ci); } }
        self.fv_changed();
    }
    fn fv_nav_step(&mut self, nav: Nav, ci: usize) {
        let Some(Some(next)) = nav.chain.get(ci + 1).cloned() else { self.fv_changed(); return };
        let col = &mut self.fv.cols[ci];
        let Some(idx) = col.entries.iter().position(|e| e.path == next) else { self.fv_changed(); return };
        for s in col.sel.iter_mut() { *s = false; }
        col.sel[idx] = true;
        col.anchor = Some(idx);
        let is_dir = col.entries[idx].is_dir;
        self.fv.focus = ci;
        self.fv_ensure_visible(ci, idx);
        if is_dir {
            let ni = self.fv_add_column(Some(next));
            self.fv.nav = Some(Nav { chain: nav.chain, step: ni });
            self.fv_scroll_end();
        }
        self.fv_changed();
    }
    /// Re-list visible columns, preserving selection by path (§9.13).
    pub fn fv_refresh(&mut self) {
        if self.fv.nav.is_some() || self.cfg.demo.is_some() { return; }
        for ci in 0..self.fv.cols.len() { self.fv_load(ci, true); }
    }
    pub fn fv_listed(&mut self, col_id: u64, seq: u64, result: Result<Vec<Entry>, String>, quiet: bool) {
        let Some(ci) = self.fv.cols.iter().position(|c| c.id == col_id) else { return };
        if self.fv.cols[ci].seq != seq { return; }
        match result {
            Ok(entries) => {
                let col = &mut self.fv.cols[ci];
                let old: Vec<String> = col.entries.iter().zip(&col.sel).filter(|(_, s)| **s).map(|(e, _)| e.path.clone()).collect();
                col.sel = entries.iter().map(|e| old.contains(&e.path)).collect();
                col.anchor = col.anchor.filter(|&a| a < entries.len());
                col.entries = entries;
                col.unreadable = false;
                if let Some(d) = self.fv.cols.get(ci + 1).and_then(|c| c.dir.clone()) {
                    if !self.fv.cols[ci].entries.iter().any(|e| e.path == d) { self.fv_truncate(ci + 1); self.fv_changed(); }
                }
                if let Some(nav) = self.fv.nav.take() { if nav.step == ci { self.fv_nav_step(nav, ci); } else { self.fv.nav = Some(nav); } }
            }
            Err(e) => {
                let col = &mut self.fv.cols[ci];
                col.entries.clear(); col.sel.clear(); col.unreadable = true;
                self.fv.nav = None;
                self.log(format!("list_dir failed: {e}"));
                if !quiet { self.show_alert("Cannot read folder", &e, &["OK"], Pending::Nothing); }
                self.fv_changed();
            }
        }
    }

    // ---- mouse
    pub fn fv_btn_hit(&self, p: Pt) -> Option<Btn> {
        let lay = self.fv_layout();
        if lay.shelf.contains(p) { return (0..self.state.shelf.len().min(16)).find(|&i| Self::shelf_item_rect(&lay, i).contains(p)).map(Btn::Shelf); }
        if lay.path.contains(p) { return (0..self.fv_path_components().len()).find(|&i| Self::path_item_rect(&lay, i).contains(p)).map(Btn::PathItem); }
        let arrow = |id: ScrollId, h: ScrollHit| match h { ScrollHit::ArrowA => Some(Btn::ScrollArrow(id, -1)), ScrollHit::ArrowB => Some(Btn::ScrollArrow(id, 1)), _ => None };
        if let Some(h) = lay.hscroll.hit(p) { return arrow(ScrollId::Browser, h); }
        if !lay.cols.contains(p) { return None; }
        for i in 0..self.fv.cols.len() {
            let (list, sc) = self.fv_col_list(&lay, i);
            if let Some(sc) = sc { if let Some(h) = sc.hit(p) { return arrow(ScrollId::Col(self.fv.cols[i].id), h); } }
            if list.contains(p) {
                let idx = (p.y - list.y + sc.map_or(0, |s| s.pos)) / CELL_H;
                return (idx >= 0 && (idx as usize) < self.fv.cols[i].entries.len()).then_some(Btn::Cell(i, idx as usize));
            }
        }
        None
    }
    pub fn fv_mouse_down(&mut self, p: Pt, mods: Mods) {
        let lay = self.fv_layout();
        if lay.shelf.contains(p) { if let Some(Btn::Shelf(i)) = self.fv_btn_hit(p) { self.capture = Some(Capture::ShelfPress { idx: i, start: p }); } return; }
        if lay.path.contains(p) { if let Some(b @ Btn::PathItem(_)) = self.fv_btn_hit(p) { self.capture = Some(Capture::Press(b)); } return; }
        if self.scroller_down(ScrollId::Browser, lay.hscroll, p) { return; }
        if !lay.cols.contains(p) { return; }
        for i in 0..self.fv.cols.len() {
            let (list, sc) = self.fv_col_list(&lay, i);
            let cid = self.fv.cols[i].id;
            if let Some(sc) = sc { if self.scroller_down(ScrollId::Col(cid), sc, p) { return; } }
            if list.contains(p) {
                let idx = (p.y - list.y + sc.map_or(0, |s| s.pos)) / CELL_H;
                if idx >= 0 && (idx as usize) < self.fv.cols[i].entries.len() { self.fv_cell_down(i, idx as usize, p, mods); }
                return;
            }
        }
    }
    fn fv_cell_down(&mut self, ci: usize, idx: usize, p: Pt, mods: Mods) {
        let col = &mut self.fv.cols[ci];
        let was = col.sel[idx];
        if let (true, Some(a)) = (mods.shift, col.anchor) {
            let (lo, hi) = (a.min(idx), a.max(idx));
            for (j, s) in col.sel.iter_mut().enumerate() { *s = j >= lo && j <= hi; }
        } else if mods.cmd {
            col.sel[idx] = !was; col.anchor = Some(idx);
        } else if !was || col.sel.iter().filter(|s| **s).count() == 1 {
            for s in col.sel.iter_mut() { *s = false; }
            col.sel[idx] = true; col.anchor = Some(idx);
        } // else: press on an already multi-selected cell keeps the selection (drag start)
        let e = col.entries[idx].clone();
        self.fv_after_select(ci);
        let now = self.now;
        let dbl = self.fv.last_click.as_ref().is_some_and(|(pp, t)| *pp == e.path && now.duration_since(*t) < Duration::from_millis(400));
        self.fv.last_click = if dbl { None } else { Some((e.path.clone(), now)) };
        if dbl && !mods.shift && !mods.cmd { if !e.is_dir { self.open_paths(&[e]); } return; }
        self.capture = Some(Capture::CellPress { col: ci, idx, start: p });
    }
    pub fn fv_start_drag(&mut self, ci: usize) {
        let sel = self.fv_selection(ci);
        if sel.is_empty() { return; }
        let icon = icons::icon_for(&sel[0].name, sel[0].is_dir, sel[0].is_app);
        self.capture = Some(Capture::FileDrag { paths: sel.into_iter().map(|e| e.path).collect(), icon, from_shelf: None });
    }
    pub fn fv_wheel(&mut self, p: Pt, dx: i32, dy: i32) {
        let lay = self.fv_layout();
        if lay.cols.contains(p) {
            for i in 0..self.fv.cols.len() {
                let (list, sc) = self.fv_col_list(&lay, i);
                if let Some(sc) = sc { if sc.r.contains(p) || list.contains(p) { self.scroll_by(ScrollId::Col(self.fv.cols[i].id), dy); return; } }
                if list.contains(p) && dx == 0 { return; }
            }
        }
        if lay.browser.contains(p) { self.scroll_by(ScrollId::Browser, if dx != 0 { dx } else { dy }); }
    }
    /// Drop into a column: onto a folder cell → that folder, else the column's directory. Alt = copy.
    pub fn fv_drop(&mut self, p: Pt, paths: Vec<String>, copy: bool) {
        let lay = self.fv_layout();
        if lay.shelf.contains(p) { for path in paths { self.shelf_add(path); } return; }
        if !lay.cols.contains(p) { return; }
        for i in 0..self.fv.cols.len() {
            let (list, sc) = self.fv_col_list(&lay, i);
            if !list.contains(p) { continue; }
            let idx = (p.y - list.y + sc.map_or(0, |s| s.pos)) / CELL_H;
            let col = &self.fv.cols[i];
            let target = match col.entries.get(idx.max(0) as usize) { Some(e) if idx >= 0 && e.is_dir => e.path.clone(), _ => match &col.dir { Some(d) => d.clone(), None => return } };
            let sep = icons::sep(&target);
            if paths.iter().any(|q| *q == target || target.starts_with(&format!("{q}{sep}")) || icons::dirname(q).as_deref() == Some(target.as_str())) { return; }
            let n = paths.len();
            let what = format!("{} {n} item(s) to {target}", if copy { "copied" } else { "moved" });
            let dest = target.clone();
            self.run_fs(what, move || if copy { fs::copy_paths(paths, dest) } else { fs::move_paths(paths, dest) });
            return;
        }
    }
    pub fn shelf_add(&mut self, p: String) {
        if self.state.shelf.contains(&p) || self.state.shelf.len() >= 16 { return; }
        self.state.shelf.push(p);
        self.dirty();
    }
    pub fn fv_select_all(&mut self) {
        let ci = self.fv.focus;
        if ci >= self.fv.cols.len() { return; }
        for s in self.fv.cols[ci].sel.iter_mut() { *s = true; }
        self.fv_after_select(ci);
    }

    // ---- keyboard (§7.8)
    pub fn fv_key(&mut self, k: Key, mods: Mods) {
        if mods.alt { return; }
        let ci = self.fv.focus;
        if ci >= self.fv.cols.len() { return; }
        let sel_idx: Vec<usize> = self.fv.cols[ci].sel.iter().enumerate().filter(|(_, s)| **s).map(|(i, _)| i).collect();
        let cur = sel_idx.last().copied();
        let n = self.fv.cols[ci].entries.len();
        match k {
            Key::Down => { if n > 0 { self.fv_pick(ci, cur.map_or(0, |c| (c + 1).min(n - 1))); } }
            Key::Up => { if n > 0 { self.fv_pick(ci, cur.map_or(0, |c| c.saturating_sub(1))); } }
            Key::Right => { if ci + 1 < self.fv.cols.len() && !self.fv.cols[ci + 1].entries.is_empty() { self.fv_pick(ci + 1, 0); } }
            Key::Left => {
                if ci > 0 {
                    self.fv.focus = ci - 1;
                    for c in self.fv.cols.iter_mut().skip(ci) { for s in c.sel.iter_mut() { *s = false; } }
                    self.fv_changed();
                }
            }
            Key::Enter => { let sel = self.fv_deep_selection(); self.open_paths(&sel); }
            Key::Delete => {
                let sel = self.fv_deep_selection();
                if sel.is_empty() { return; }
                let what = if sel.len() == 1 { format!("“{}”", sel[0].name) } else { format!("{} items", sel.len()) };
                self.show_alert(&format!("Move {what} to the Recycler?"), "", &["Cancel", "Move"], Pending::Trash(sel.iter().map(|e| e.path.clone()).collect()));
            }
            Key::Escape => {}
            Key::Char(c) => {
                self.fv.typeahead.push(c.to_ascii_lowercase());
                self.fv.type_until = Some(self.now + Duration::from_millis(700));
                let t = self.fv.typeahead.clone();
                if let Some(j) = self.fv.cols[ci].entries.iter().position(|e| e.name.to_lowercase().starts_with(&t)) { self.fv_pick(ci, j); }
            }
        }
    }
    fn fv_pick(&mut self, ci: usize, idx: usize) {
        let col = &mut self.fv.cols[ci];
        if idx >= col.entries.len() { return; }
        for s in col.sel.iter_mut() { *s = false; }
        col.sel[idx] = true;
        col.anchor = Some(idx);
        self.fv_ensure_visible(ci, idx);
        self.fv_after_select(ci);
    }

    // ---- drawing
    pub fn fv_draw(&self, p: &mut Painter, _c: Rect) {
        let lay = self.fv_layout();
        let pressed = self.pressed();
        p.raised(lay.shelf);
        for (i, path) in self.state.shelf.iter().take(16).enumerate() {
            let r = Self::shelf_item_rect(&lay, i);
            p.icon(self.fv_path_icon(path), r.x + 8, r.y + 8, 48);
            let t = p.ellipsize_mid(FontId::Regular, 10, &icons::basename(path), 62);
            p.text_in(FontId::Regular, 10, rect(r.x + 1, r.y + 59, 62, 12), Align::Center, &t, BLACK);
        }
        let comps = self.fv_path_components();
        let leaf = self.fv_sel_entry();
        for (i, c) in comps.iter().enumerate() {
            let r = Self::path_item_rect(&lay, i);
            let last = i + 1 == comps.len();
            let icon = if *c == self.home { "home" } else if icons::is_root(c) { "drive" } else if last && leaf.as_ref().is_some_and(|e| !e.is_dir) { let e = leaf.as_ref().unwrap(); icons::icon_for(&e.name, false, e.is_app) } else { "folder" };
            p.icon(icon, r.x + 8, r.y + 2, 48);
            let t = p.ellipsize_mid(FontId::Regular, 10, &icons::basename(c), 62);
            p.text_in(FontId::Regular, 10, rect(r.x + 1, r.y + 52, 62, 12), Align::Center, &t, BLACK);
        }
        p.sunken(lay.browser);
        p.push_clip(lay.cols);
        for i in 0..self.fv.cols.len() {
            let cr = self.fv_col_rect(&lay, i);
            if cr.right() < lay.cols.x || cr.x > lay.cols.right() { continue; }
            p.vline(cr.right() - 1, cr.y, cr.h, DARK);
            let (list, sc) = self.fv_col_list(&lay, i);
            let col = &self.fv.cols[i];
            p.push_clip(list);
            if col.unreadable { p.text(FontId::Regular, 12, list.x + 6, list.y + 14, "(unreadable)", DARK); }
            let scroll = sc.map_or(0, |s| s.pos);
            let first = (scroll / CELL_H).max(0) as usize;
            for (idx, e) in col.entries.iter().enumerate().skip(first).take((list.h / CELL_H + 2) as usize) {
                let r = Self::cell_rect(list, scroll, idx);
                let selected = col.sel.get(idx).copied().unwrap_or(false);
                if selected { p.fill(r, BLACK); }
                let fg = if selected { WHITE } else { BLACK };
                p.icon(icons::small_icon_for(&e.name, e.is_dir, e.is_app), r.x + 2, r.y + 1, 16);
                if e.is_symlink { p.icon("symlink-badge", r.x + 10, r.y + 7, 12); }
                let name_w = r.w - 22 - if e.is_dir { 14 } else { 4 };
                let name = p.ellipsize_mid(FontId::Regular, 12, &e.name, name_w);
                p.text_in(FontId::Regular, 12, rect(r.x + 22, r.y, name_w, r.h), Align::Left, &name, fg);
                if e.is_dir { tri_right(p, r.right() - 8, r.y + 6, 6, fg); }
            }
            p.pop_clip();
            if let Some(sc) = sc {
                let pr = match pressed { Some(Btn::ScrollArrow(ScrollId::Col(id), d)) if id == col.id => Some(if d < 0 { ScrollHit::ArrowA } else { ScrollHit::ArrowB }), _ => None };
                sc.draw(p, pr);
            }
        }
        p.pop_clip();
        let pr = match pressed { Some(Btn::ScrollArrow(ScrollId::Browser, d)) => Some(if d < 0 { ScrollHit::ArrowA } else { ScrollHit::ArrowB }), _ => None };
        lay.hscroll.draw(p, pr);
    }
}
