//! Dock (§7.5), application tile (§7.6), Recycler tile and window (§7.7).
use crate::app::*;
use crate::backend::{apps, fs};
use crate::backend::fs::Entry;
use crate::chrome::*;
use crate::geom::{rect, Pt, Rect};
use crate::icons;
use crate::paint::*;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TileId { Workspace, App(usize), Recycler }

pub struct Dock {
    pub trash_empty: bool,
    pub warned: bool,
    pub pressed_until: Option<(usize, Instant)>,
    pub requested: HashSet<String>,
    pub icons: HashMap<String, Rc<Image>>,
    pub last_mini_click: Option<(WinKind, Instant)>,
}
impl Default for Dock {
    fn default() -> Self { Dock { trash_empty: true, warned: false, pressed_until: None, requested: HashSet::new(), icons: HashMap::new(), last_mini_click: None } }
}

#[derive(Default)]
pub struct RecWin { pub items: Option<Result<Vec<Entry>, String>>, pub scroll: i32 }

fn tile(p: &mut Painter, r: Rect, pressed: bool) { if pressed { p.sunken(r) } else { tile_bevel(p, r) } }
fn dots(p: &mut Painter, r: Rect) { for i in 0..3 { p.fill(rect(r.x + 7 + i * 4, r.y + 55, 2, 2), BLACK); } }

impl App {
    /// Tiles hug the right edge with the 3 px margin seen on real screens.
    pub fn dock_strip(&self) -> Rect { rect(self.w - 67, 0, 67, self.h - 64) }
    pub fn tile_rect(&self, t: TileId) -> Rect {
        match t { TileId::Workspace => rect(self.w - 67, 0, 64, 64), TileId::App(i) => rect(self.w - 67, 64 * (i as i32 + 1), 64, 64), TileId::Recycler => rect(self.w - 67, self.h - 64, 64, 64) }
    }
    fn apptile_rect(&self) -> Rect { rect(0, self.h - 64, 64, 64) }
    /// Tiles float above windows: dock tiles, Recycler, application tile, miniwindows.
    pub fn tile_hit(&self, p: Pt) -> Option<Btn> {
        if self.tile_rect(TileId::Recycler).contains(p) { return Some(Btn::Tile(TileId::Recycler)); }
        if self.state.dock_visible && self.dock_strip().contains(p) {
            if self.tile_rect(TileId::Workspace).contains(p) { return Some(Btn::Tile(TileId::Workspace)); }
            return (0..self.state.dock.len().min(12)).find(|&i| self.tile_rect(TileId::App(i)).contains(p)).map(|i| Btn::Tile(TileId::App(i)));
        }
        if self.apptile_rect().contains(p) { return Some(Btn::Tile(TileId::Workspace)); }
        for w in &self.wins { if let Some(slot) = w.mini { if miniwindow_rect(slot, self.h).contains(p) { return Some(Btn::Miniwin(w.kind)); } } }
        None
    }
    pub fn draw_dock(&self, p: &mut Painter) {
        let pressed = self.pressed();
        let ws = self.tile_rect(TileId::Workspace);
        tile(p, ws, pressed == Some(Btn::Tile(TileId::Workspace)));
        p.icon("workspace", ws.x + 8, ws.y + 8, 48);
        let (drag_idx, dx, dy) = match &self.capture { Some(Capture::TilePress { idx, dx, dy, .. }) => (Some(*idx), *dx, *dy), _ => (None, 0, 0) };
        for (i, app) in self.state.dock.iter().take(12).enumerate() {
            if drag_idx == Some(i) { continue; }
            self.draw_app_tile(p, i, app, self.tile_rect(TileId::App(i)), pressed);
        }
        if let Some(i) = drag_idx { if let Some(app) = self.state.dock.get(i) { let r = self.tile_rect(TileId::App(i)); self.draw_app_tile(p, i, app, r.at(dx, dy), pressed); } }
    }
    pub fn draw_recycler_tile(&self, p: &mut Painter) {
        let rc = self.tile_rect(TileId::Recycler);
        tile(p, rc, self.pressed() == Some(Btn::Tile(TileId::Recycler)));
        p.icon(if self.dock.trash_empty { "recycler-empty" } else { "recycler-full" }, rc.x + 8, rc.y + 8, 48);
    }
    pub fn draw_apptile(&self, p: &mut Painter) {
        let at = self.apptile_rect();
        tile_bevel(p, at);
        p.icon("workspace", at.x + 8, at.y + 8, 48);
    }
    fn draw_app_tile(&self, p: &mut Painter, i: usize, app: &str, r: Rect, pressed: Option<Btn>) {
        let down = pressed == Some(Btn::Tile(TileId::App(i))) || self.dock.pressed_until.is_some_and(|(j, _)| j == i);
        tile(p, r, down);
        match self.dock.icons.get(app) { Some(img) => p.image(img, r.x + 8, r.y + 8, Some((48, 48)), 255), None => p.icon("application", r.x + 8, r.y + 8, 48) }
        dots(p, r);
    }
    /// Request host app icons that are not cached yet (called from the draw path's data side: start()/dock changes).
    pub fn dock_request_icons(&mut self) {
        if !self.state.dock_visible { return; }
        for app in self.state.dock.clone() {
            if self.dock.icons.contains_key(&app) || self.dock.requested.contains(&app) { continue; }
            self.dock.requested.insert(app.clone());
            self.spawn(move || Job::AppIcon { result: apps::app_icon_png(&app, 96), app });
        }
    }
    pub fn dock_icon(&mut self, app: String, result: Result<Vec<u8>, String>) {
        match result.and_then(|b| decode_png(&b)) {
            Ok(img) => { self.dock.icons.insert(app, Rc::new(img)); }
            Err(e) => self.log(format!("icon for {}: {e}", icons::basename(&app))),
        }
    }
    pub fn dock_tile_click(&mut self, t: TileId) {
        match t {
            TileId::Workspace => self.show_win(WinKind::FileViewer),
            TileId::Recycler => self.show_win(WinKind::Recycler),
            TileId::App(i) => self.dock_launch(i),
        }
    }
    fn dock_launch(&mut self, i: usize) {
        let Some(app) = self.state.dock.get(i).cloned() else { return };
        self.dock.pressed_until = Some((i, self.now + Duration::from_millis(150)));
        match apps::launch_app(&app) { Ok(()) => self.log(format!("launched {}", icons::basename(&app))), Err(e) => self.error(&format!("Cannot launch {}", icons::basename(&app)), e) }
    }
    /// Vertical drag reorders, more than 48 px sideways removes, no movement = click (§7.4).
    pub fn dock_tile_release(&mut self, i: usize, dx: i32, dy: i32) {
        if i >= self.state.dock.len() { return; }
        if dx.abs() <= 4 && dy.abs() <= 4 { self.dock_launch(i); return; }
        if dx.abs() > 48 { let app = self.state.dock.remove(i); self.log(format!("removed {} from the Dock", icons::basename(&app))); }
        else {
            let n = self.state.dock.len() as i32;
            let j = ((64 * (i as i32 + 1) + dy + 32) / 64 - 1).clamp(0, n - 1) as usize;
            if j != i { let app = self.state.dock.remove(i); self.state.dock.insert(j, app); }
        }
        self.dirty();
    }
    pub fn dock_add(&mut self, p: String) {
        if self.state.dock.contains(&p) || self.state.dock.len() >= 12 { return; }
        self.log(format!("added {} to the Dock", icons::basename(&p)));
        self.state.dock.push(p);
        self.dirty();
        self.dock_request_icons();
    }
    pub fn dock_trash_state(&mut self, r: Result<bool, String>) {
        match r {
            Ok(empty) => self.dock.trash_empty = empty,
            Err(e) => { if !self.dock.warned { self.dock.warned = true; self.log(format!("trash_is_empty: {e}")); } self.dock.trash_empty = true; }
        }
        if self.win(WinKind::Recycler).shown() && self.rec.items.is_none() { self.rec_open(); }
    }

    // ---- Recycler window
    pub fn rec_open(&mut self) {
        if cfg!(target_os = "macos") { self.rec.items = None; self.spawn(|| Job::TrashList(fs::trash_list())); }
    }
    pub fn rec_scroller(&self) -> Scroller {
        let c = self.content_rect(WinKind::Recycler);
        let n = self.rec.items.as_ref().and_then(|r| r.as_ref().ok()).map_or(0, |v| v.len()) as i32;
        let per_row = ((c.w - SCROLL_W - 8) / 80).max(1);
        let total = ((n + per_row - 1) / per_row) * 90 + 8;
        Scroller { r: rect(c.x, c.y, SCROLL_W, c.h), vertical: true, total, visible: c.h, pos: self.rec.scroll.clamp(0, (total - c.h).max(0)) }
    }
    fn rec_button_rect(&self) -> Rect { let c = self.content_rect(WinKind::Recycler); rect(c.x + SCROLL_W + 12, c.y + 60, 120, BTN_H - 1) }
    fn rec_has_button(&self) -> bool { !cfg!(target_os = "macos") || matches!(self.rec.items, Some(Err(_))) }
    pub fn rec_btn_hit(&self, p: Pt) -> Option<Btn> {
        if self.rec_has_button() && self.rec_button_rect().contains(p) { return Some(Btn::RecBtn); }
        match self.rec_scroller().hit(p)? { ScrollHit::ArrowA => Some(Btn::ScrollArrow(ScrollId::Recycler, -1)), ScrollHit::ArrowB => Some(Btn::ScrollArrow(ScrollId::Recycler, 1)), _ => None }
    }
    pub fn rec_mouse_down(&mut self, p: Pt) {
        if self.rec_has_button() && self.rec_button_rect().contains(p) { self.capture = Some(Capture::Press(Btn::RecBtn)); return; }
        let sc = self.rec_scroller();
        self.scroller_down(ScrollId::Recycler, sc, p);
    }
    pub fn rec_button(&mut self) {
        if cfg!(target_os = "macos") {
            let t = icons::join(&self.home, ".Trash");
            if let Err(e) = apps::open_path(&t) { self.log(format!("open .Trash: {e}")); }
        } else if let Err(e) = apps::open_recycle_bin() { self.log(format!("open_recycle_bin: {e}")); }
    }
    pub fn rec_draw(&self, p: &mut Painter, c: Rect) {
        let pressed = self.pressed();
        let c = rect(c.x + SCROLL_W, c.y, c.w - SCROLL_W, c.h);
        if !cfg!(target_os = "macos") {
            p.text(FontId::Regular, 12, c.x + 12, c.y + 24, "The Recycle Bin is not a folder on Windows.", BLACK);
            button(p, self.rec_button_rect(), "Open Recycle Bin", pressed == Some(Btn::RecBtn), false);
            return;
        }
        match &self.rec.items {
            None => { p.text(FontId::Regular, 12, c.x + 12, c.y + 24, "…", DARK); }
            Some(Err(e)) => {
                p.text(FontId::Regular, 12, c.x + 12, c.y + 20, "Cannot list the Recycler.", BLACK);
                let e = p.ellipsize(FontId::Regular, 10, e, c.w - 24);
                p.text(FontId::Regular, 10, c.x + 12, c.y + 34, &e, DARK);
                p.text(FontId::Regular, 10, c.x + 12, c.y + 48, "Grant Full Disk Access in System Settings, or:", DARK);
                button(p, self.rec_button_rect(), "Open in Finder", pressed == Some(Btn::RecBtn), false);
            }
            Some(Ok(items)) => {
                let sc = self.rec_scroller();
                if items.is_empty() { p.text(FontId::Regular, 12, c.x + 12, c.y + 24, "The Recycler is empty.", DARK); }
                let per_row = ((c.w - 8) / 80).max(1);
                p.push_clip(c);
                for (i, e) in items.iter().enumerate() {
                    let (col, row) = (i as i32 % per_row, i as i32 / per_row);
                    let r = rect(c.x + 4 + col * 80, c.y + 4 + row * 90 - sc.pos, 80, 90);
                    if r.bottom() < c.y || r.y > c.bottom() { continue; }
                    p.icon(icons::icon_for(&e.name, e.is_dir, e.is_app), r.x + 16, r.y + 8, 48);
                    let t = p.ellipsize_mid(FontId::Regular, 10, &e.name, 76);
                    p.text_in(FontId::Regular, 10, rect(r.x + 2, r.y + 66, 76, 12), Align::Center, &t, BLACK);
                }
                p.pop_clip();
                let pr = match pressed { Some(Btn::ScrollArrow(ScrollId::Recycler, d)) => Some(if d < 0 { ScrollHit::ArrowA } else { ScrollHit::ArrowB }), _ => None };
                sc.draw(p, pr);
            }
        }
    }
}
