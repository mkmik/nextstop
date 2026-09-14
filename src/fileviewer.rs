//! File Viewer (§7.8): Shelf, Icon Path, column Browser in the NeXTSTEP 1.0 style. Implemented as methods on App.
use crate::app::*;
use crate::backend::fs::{self, Entry};
use crate::chrome::*;
use crate::geom::{rect, Pt, Rect};
use crate::icons;
use crate::paint::*;
use std::path::Path;
use std::time::{Duration, Instant};

pub const STRIP_W: i32 = 17;                            // per-column scroller strip
pub const LIST_W: i32 = 121;                            // column list width
pub const COL_PITCH: i32 = STRIP_W + 2 + LIST_W + 1;     // strip, light + black line, list, black separator = 141
pub const CELL_H: i32 = 15;
pub const SHELF_H: i32 = 88;                            // icons, 12 px labels, free-space line
pub const WELL_ICONS_H: i32 = 79;                       // icon-path area inside the well
pub const HSCROLL_H: i32 = 17;                          // horizontal scroller inside the well
pub const SHELF_CELL: i32 = 96;

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
    pub disk_free: Option<String>,
    pub cols: Vec<Column>,
    pub focus: usize,
    pub hscroll: i32,
    pub typeahead: String,
    pub type_until: Option<Instant>,
    pub last_click: Option<(String, Instant)>,
    pub nav: Option<Nav>,
    pub next_id: u64,
}

pub struct FvLayout { pub shelf: Rect, pub well: Rect, pub icons: Rect, pub hscroll: Scroller, pub browser: Rect, pub cols: Rect }
/// One column: its scroller strip and the list area.
pub struct ColParts { pub strip: Scroller, pub list: Rect }

impl App {
    // ---- layout (NeXTSTEP 2.0 File Viewer: shelf, icon-path well with the column scroller, browser)
    pub fn fv_layout(&self) -> FvLayout {
        let c = self.content_rect(WinKind::FileViewer);
        let shelf = rect(c.x, c.y, c.w, SHELF_H);
        let well = rect(c.x + 8, c.y + SHELF_H, c.w - 16, 2 + WELL_ICONS_H + 1 + HSCROLL_H + 1);
        let icons = rect(well.x + 2, well.y + 2, well.w - 3, WELL_ICONS_H);
        let browser = rect(c.x + 8, well.bottom() + 5, c.w - 16, c.h - SHELF_H - well.h - 9);
        let cols = rect(browser.x + 2, browser.y + 2, browser.w - 3, browser.h - 3);
        let total = self.fv.cols.len() as i32 * COL_PITCH;
        let pos = self.fv.hscroll.clamp(0, (total - cols.w).max(0));
        let hscroll = Scroller { r: rect(well.x + 2, icons.bottom() + 1, well.w - 3, HSCROLL_H), vertical: false, frame: false, arrows: false, total, visible: cols.w, pos };
        FvLayout { shelf, well, icons, hscroll, browser, cols }
    }
    fn shelf_item_rect(lay: &FvLayout, i: usize) -> Rect { rect(lay.shelf.x + 8 + i as i32 * SHELF_CELL, lay.shelf.y + 4, SHELF_CELL, 72) }
    fn col_x(lay: &FvLayout, i: usize) -> i32 { lay.cols.x - lay.hscroll.pos + i as i32 * COL_PITCH }
    /// Icon of path component `i` sits centred above column `i` (the leaf file above the column that would follow).
    fn path_item_rect(lay: &FvLayout, i: usize) -> Rect { rect(Self::col_x(lay, i) + (COL_PITCH - 48) / 2, lay.icons.y + 4, 48, 48) }
    pub fn fv_col_parts(&self, lay: &FvLayout, i: usize) -> ColParts {
        let x = Self::col_x(lay, i);
        let col = &self.fv.cols[i];
        let total = col.entries.len() as i32 * CELL_H;
        let strip = Scroller { r: rect(x, lay.cols.y, STRIP_W, lay.cols.h), vertical: true, frame: false, arrows: true, total, visible: lay.cols.h, pos: col.scroll.clamp(0, (total - lay.cols.h).max(0)) };
        ColParts { strip, list: rect(x + STRIP_W + 2, lay.cols.y, LIST_W, lay.cols.h) }
    }
    pub fn fv_col_scroller(&self, cid: u64) -> Option<Scroller> {
        let i = self.fv.cols.iter().position(|c| c.id == cid)?;
        let lay = self.fv_layout();
        Some(self.fv_col_parts(&lay, i).strip)
    }
    pub fn col_scroll(&self, lay: &FvLayout, i: usize) -> i32 { self.fv_col_parts(lay, i).strip.pos }
    fn cell_rect(list: Rect, scroll: i32, idx: usize) -> Rect { rect(list.x, list.y + idx as i32 * CELL_H - scroll, list.w, CELL_H) }
    pub fn fv_cell_rect_of(&self, ci: usize, idx: usize) -> Option<Rect> {
        let lay = self.fv_layout();
        if idx >= self.fv.cols.get(ci)?.entries.len() { return None; }
        Some(Self::cell_rect(self.fv_col_parts(&lay, ci).list, self.col_scroll(&lay, ci), idx))
    }
    pub fn fv_sel_cell_rect(&self) -> Option<Rect> {
        let ci = self.fv.focus;
        let idx = self.fv.cols.get(ci)?.sel.iter().position(|s| *s)?;
        self.fv_cell_rect_of(ci, idx)
    }

    // ---- selection queries
    pub fn fv_selection(&self, ci: usize) -> Vec<Entry> {
        self.fv.cols.get(ci).map_or(vec![], |c| c.entries.iter().zip(&c.sel).filter(|(_, s)| **s).map(|(e, _)| e.clone()).collect())
    }
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
        let h = self.fv_col_parts(&lay, ci).list.h;
        let col = &mut self.fv.cols[ci];
        let top = idx as i32 * CELL_H;
        if top < col.scroll { col.scroll = top; } else if top + CELL_H > col.scroll + h { col.scroll = top + CELL_H - h; }
    }
    fn fv_changed(&mut self) {
        self.fv_update_disk_free();
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
        self.fv_update_disk_free();
    }
    fn fv_update_disk_free(&mut self) {
        let dir = self.fv_current_dir();
        self.fv.disk_free = fs::disk_free(&dir).map(|b| format!("{}MB available on hard disk", b / 1_000_000));
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
        if lay.icons.contains(p) { return (0..self.fv_path_components().len()).find(|&i| Self::path_item_rect(&lay, i).inset(-8).contains(p)).map(Btn::PathItem); }
        if !lay.cols.contains(p) { return None; }
        for i in 0..self.fv.cols.len() {
            let parts = self.fv_col_parts(&lay, i);
            let id = self.fv.cols[i].id;
            if let Some(h) = parts.strip.hit(p) {
                return match h { ScrollHit::ArrowA => Some(Btn::ScrollArrow(ScrollId::Col(id), -1)), ScrollHit::ArrowB => Some(Btn::ScrollArrow(ScrollId::Col(id), 1)), _ => None };
            }
            if parts.list.contains(p) {
                let idx = (p.y - parts.list.y + parts.strip.pos) / CELL_H;
                return (idx >= 0 && (idx as usize) < self.fv.cols[i].entries.len()).then_some(Btn::Cell(i, idx as usize));
            }
        }
        None
    }
    pub fn fv_mouse_down(&mut self, p: Pt, mods: Mods) {
        let lay = self.fv_layout();
        if lay.shelf.contains(p) { if let Some(Btn::Shelf(i)) = self.fv_btn_hit(p) { self.capture = Some(Capture::ShelfPress { idx: i, start: p }); } return; }
        if lay.hscroll.r.contains(p) { self.scroller_down(ScrollId::Browser, lay.hscroll, p); return; }
        if lay.cols.contains(p) {
            for i in 0..self.fv.cols.len() {
                let parts = self.fv_col_parts(&lay, i);
                if parts.strip.r.contains(p) { let id = self.fv.cols[i].id; self.scroller_down(ScrollId::Col(id), parts.strip, p); return; }
            }
        }
        match self.fv_btn_hit(p) {
            Some(Btn::Cell(ci, idx)) => self.fv_cell_down(ci, idx, p, mods),
            Some(b) => self.capture = Some(Capture::Press(b)),
            None => {}
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
        }
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
                let parts = self.fv_col_parts(&lay, i);
                if parts.list.contains(p) || parts.strip.r.contains(p) { if dx.abs() > dy.abs() { self.scroll_by(ScrollId::Browser, dx); } else { self.scroll_by(ScrollId::Col(self.fv.cols[i].id), dy); } return; }
            }
        }
        if lay.browser.contains(p) || lay.well.contains(p) { self.scroll_by(ScrollId::Browser, if dx != 0 { dx } else { dy }); }
    }
    /// Drop into a column: onto a folder cell → that folder, else the column's directory. Alt = copy.
    pub fn fv_drop(&mut self, p: Pt, paths: Vec<String>, copy: bool) {
        let lay = self.fv_layout();
        if lay.shelf.contains(p) { for path in paths { self.shelf_add(path); } return; }
        if !lay.cols.contains(p) { return; }
        for i in 0..self.fv.cols.len() {
            let parts = self.fv_col_parts(&lay, i);
            if !parts.list.contains(p) { continue; }
            let idx = (p.y - parts.list.y + parts.strip.pos) / CELL_H;
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
            Key::Delete | Key::Backspace => {
                let sel = self.fv_deep_selection();
                if sel.is_empty() { return; }
                let what = if sel.len() == 1 { format!("“{}”", sel[0].name) } else { format!("{} items", sel.len()) };
                self.show_alert(&format!("Move {what} to the Recycler?"), "", &["Cancel", "Move"], Pending::Trash(sel.iter().map(|e| e.path.clone()).collect()));
            }
            Key::Escape | Key::Tab | Key::Home | Key::End | Key::PageUp | Key::PageDown => {}
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
        // Shelf: icons with 12 px labels straight on the window face, free space below
        for (i, path) in self.state.shelf.iter().take(16).enumerate() {
            let r = Self::shelf_item_rect(&lay, i);
            p.icon(self.fv_path_icon(path), r.x + 24, r.y + 4, 48);
            let t = p.ellipsize_mid(FontId::Regular, 12, &icons::basename(path), r.w - 6);
            p.text_in(FontId::Regular, 12, rect(r.x + 3, r.y + 54, r.w - 6, 14), Align::Center, &t, BLACK);
        }
        if let Some(t) = &self.fv.disk_free { p.text(FontId::Regular, 10, lay.shelf.x + 8, lay.shelf.bottom() - 5, t, BLACK); }
        // Icon-path well: sunken frame, icons above their columns, ▷ between them, white box on the leaf
        let w = lay.well;
        p.fill(w, LIGHT);
        p.hline(w.x, w.y, w.w, DARK); p.hline(w.x + 1, w.y + 1, w.w - 1, BLACK); p.vline(w.x, w.y, w.h, DARK); p.vline(w.x + 1, w.y + 1, w.h - 1, BLACK);
        p.hline(w.x, w.bottom() - 1, w.w, WHITE); p.vline(w.right() - 1, w.y, w.h, WHITE);
        p.hline(lay.icons.x, lay.icons.bottom(), lay.icons.w, BLACK);
        let comps = self.fv_path_components();
        let leaf = self.fv_sel_entry();
        p.push_clip(lay.icons);
        for (i, c) in comps.iter().enumerate() {
            let r = Self::path_item_rect(&lay, i);
            if r.right() < lay.icons.x || r.x > lay.icons.right() { continue; }
            let last = i + 1 == comps.len();
            let icon = if *c == self.home { "home" } else if icons::is_root(c) { "computer" } else if last && leaf.as_ref().is_some_and(|e| !e.is_dir) { let e = leaf.as_ref().unwrap(); icons::icon_for(&e.name, false, e.is_app) } else { "folder" };
            let name = icons::basename(c);
            let label = if icons::is_root(c) { String::new() } else { p.ellipsize_mid(FontId::Regular, 12, &name, COL_PITCH - 20) };
            let lw = p.text_width(FontId::Regular, 12, &label);
            if last {
                p.fill(rect(r.x - 8, r.y - 4, 64, 56), WHITE);
                if !label.is_empty() { p.fill(rect(r.x + 24 - lw / 2 - 4, r.y + 52, lw + 8, 15), WHITE); }
            }
            p.icon(icon, r.x, r.y, 48);
            if !label.is_empty() { p.text_in(FontId::Regular, 12, rect(r.x + 24 - lw / 2 - 4, r.y + 52, lw + 8, 15), Align::Center, &label, BLACK); }
            if i > 0 { tri_hollow_right(p, Self::col_x(&lay, i) - 5, r.y + 20, BLACK); }
        }
        p.pop_clip();
        let pr = match pressed { Some(Btn::ScrollArrow(ScrollId::Browser, d)) => Some(if d < 0 { ScrollHit::ArrowA } else { ScrollHit::ArrowB }), _ => None };
        lay.hscroll.draw(p, pr);
        // Browser: sunken box; each column is a scroller strip plus a list, separated by black lines
        let b = lay.browser;
        p.fill(b, LIGHT);
        p.hline(b.x, b.y, b.w, DARK); p.hline(b.x + 1, b.y + 1, b.w - 1, BLACK); p.vline(b.x, b.y, b.h, DARK); p.vline(b.x + 1, b.y + 1, b.h - 1, BLACK);
        p.hline(b.x, b.bottom() - 1, b.w, WHITE); p.vline(b.right() - 1, b.y, b.h, WHITE);
        p.push_clip(lay.cols);
        for i in 0..self.fv.cols.len() {
            let parts = self.fv_col_parts(&lay, i);
            if parts.list.right() < lay.cols.x || parts.strip.r.x > lay.cols.right() { continue; }
            let col = &self.fv.cols[i];
            let pr = match pressed { Some(Btn::ScrollArrow(ScrollId::Col(id), d)) if id == col.id => Some(if d < 0 { ScrollHit::ArrowA } else { ScrollHit::ArrowB }), _ => None };
            parts.strip.draw(p, pr);
            let sx = parts.strip.r.right();
            p.vline(sx, lay.cols.y, lay.cols.h, LIGHT); p.vline(sx + 1, lay.cols.y, lay.cols.h, BLACK);
            p.vline(parts.list.right(), lay.cols.y, lay.cols.h, BLACK);
            p.push_clip(parts.list);
            if col.unreadable { p.text(FontId::Regular, 12, parts.list.x + 4, parts.list.y + 13, "(unreadable)", DARK); }
            let scroll = parts.strip.pos;
            let first = (scroll / CELL_H).max(0) as usize;
            for (idx, e) in col.entries.iter().enumerate().skip(first).take((parts.list.h / CELL_H + 2) as usize) {
                let r = Self::cell_rect(parts.list, scroll, idx);
                if col.sel.get(idx).copied().unwrap_or(false) { p.fill(r, WHITE); }
                let mut right = r.right() - 3;
                if e.is_dir { right -= 6; tri_hollow_right(p, right + 1, r.y + 4, BLACK); right -= 3; }
                if e.is_symlink { right -= 12; p.icon("symlink-badge", right, r.y + 2, 12); right -= 2; }
                let name_w = right - (r.x + 3);
                p.text_in(FontId::Regular, 12, rect(r.x + 3, r.y, name_w, r.h), Align::Left, &p.ellipsize_mid(FontId::Regular, 12, &e.name, name_w), BLACK);
            }
            p.pop_clip();
        }
        p.pop_clip();
    }
}
