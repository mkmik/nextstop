//! Preferences — the 1990 application: a row of module icons over the pane of the selected one.
//! Geometry measured from toastytech's `ns10prefs.png` (see docs/DECISIONS.md); the switch and its
//! check mark come from `ns09prefs.png`.
use crate::app::*;
use crate::chrome::*;
use crate::geom::{rect, Pt, Rect};
use crate::paint::*;

pub const WIN_W: i32 = 392;
pub const WIN_H: i32 = 319; // the 1.0 window, title bar included
const MARGIN: i32 = 8;
const BOX_H: i32 = 90; // 2 px frame + 65 px of cells + the 21 px scroller + the bottom edge
const CELL_W: i32 = 69; // 2 px separator + 1 px highlight + a 66 px face
const CELL_H: i32 = 65;
const ROW_H: i32 = 22; // one switch and its title
const DISPLAY: usize = 0; // the module with the scale slider instead of switches
const EXPERT: usize = 2;  // the one that also says where the settings are kept
const SCALE_MIN: f32 = 0.5;
const SCALE_MAX: f32 = 4.0;
const SCALE_STEP: f32 = 0.25;

/// One module: its icon in the row at the top, its name and its switches. A switch is a View menu
/// command through the same call, so the switch and the menu item cannot disagree — `act_state`
/// draws both (§9.2, AGENTS.md).
pub struct Module { pub icon: &'static str, pub name: &'static str, pub group: &'static str, pub switches: &'static [(&'static str, Act)] }

pub static MODULES: [Module; 3] = [
    Module { icon: "computer", name: "Display", group: "Screen Scale", switches: &[] },
    Module { icon: "workspace", name: "Workspace", group: "Show on the Screen", switches: &[
        ("Show Dock", Act::ShowDock), ("Show Miniwindows", Act::ShowMiniwindows),
        ("Show Recycler Tile", Act::ShowRecycler), ("Screen Backdrop", Act::Backdrop),
    ] },
    Module { icon: "file-code", name: "Expert", group: "File Viewer", switches: &[("Show Hidden Files", Act::ShowHidden)] },
];

#[derive(Default)]
pub struct Prefs {
    pub module: usize,
    /// The scale under the knob while it is being dragged; applied when the mouse is let go.
    // ponytail: a live slider fights itself — every new factor resizes the window under the
    // pointer, so the knob runs away from it. Commit on release, as the original's Set button did.
    pub drag: Option<f32>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PBtn { Module(usize), Switch(usize) }

/// The 1.0 switch: a 15 px light square, white on its top and left edge, black on the other two,
/// with a check mark in white and its black shadow when it is on.
fn switch(p: &mut Painter, r: Rect, on: bool) {
    p.fill(r, LIGHT);
    p.bevel(r, WHITE, BLACK);
    if !on { return; }
    const CHECK: [&str; 7] = ["WK...WK", "WK..WK.", "WK.WK..", "W.WK...", "WWK....", "WK.....", "K......"];
    for (dy, row) in CHECK.iter().enumerate() {
        for (dx, ch) in row.chars().enumerate() {
            let c = match ch { 'W' => WHITE, 'K' => BLACK, _ => continue };
            p.fill(rect(r.x + 3 + dx as i32, r.y + 5 + dy as i32, 1, 1), c);
        }
    }
}

/// Slider: the scroller's dithered slot with its raised, dimpled knob riding in it.
fn slider(p: &mut Painter, r: Rect, frac: f32) {
    p.fill(r, LIGHT);
    p.bevel(r, DARK, WHITE);
    let t = r.inset(1);
    p.dither(t);
    let k = rect(t.x + ((t.w - 16) as f32 * frac.clamp(0.0, 1.0)).round() as i32, t.y, 15, t.h - 1);
    p.raised(k);
    dimple(p, k.x + (k.w - 6) / 2, k.y + (k.h - 6) / 2);
}

impl App {
    fn pref_box(&self) -> Rect { let c = self.content_rect(WinKind::Preferences); rect(c.x + MARGIN, c.y + 6, c.w - 2 * MARGIN, BOX_H) }
    fn pref_cell(&self, i: usize) -> Rect { let b = self.pref_box(); rect(b.x + 2 + i as i32 * CELL_W, b.y + 2, CELL_W, CELL_H) }
    fn pref_pane(&self) -> Rect {
        let c = self.content_rect(WinKind::Preferences);
        let y = c.y + 6 + BOX_H + 9; // the groove under the icon row sits in the 9 px gap
        rect(c.x, y, c.w, c.bottom() - y)
    }
    fn pref_group(&self) -> Rect {
        let p = self.pref_pane();
        let rows = MODULES[self.prefs.module].switches.len() as i32;
        let h = if rows == 0 { 76 } else { 18 + rows * ROW_H };
        rect(p.x + 16, p.y + 40, p.w - 32, h)
    }
    fn pref_switch(&self, i: usize) -> Rect { let g = self.pref_group(); rect(g.x + 14, g.y + 12 + i as i32 * ROW_H, g.w - 28, 15) }
    fn pref_slider(&self) -> Rect { let g = self.pref_group(); rect(g.x + 16, g.y + 24, 228, 18) }
    fn pref_field(&self) -> Rect { let s = self.pref_slider(); rect(s.right() + 16, s.y - 1, 62, 20) }
    /// The factor the knob stands for at `x`, snapped to a quarter.
    fn pref_scale_at(&self, x: i32) -> f32 {
        let t = self.pref_slider().inset(1);
        let frac = ((x - t.x - 8) as f32 / (t.w - 16).max(1) as f32).clamp(0.0, 1.0);
        let steps = ((SCALE_MAX - SCALE_MIN) / SCALE_STEP).round();
        SCALE_MIN + (frac * steps).round() * SCALE_STEP
    }
    /// The factor the window shows: the one being dragged, else the one in force.
    pub fn pref_scale(&self) -> f32 { self.prefs.drag.unwrap_or(self.zoom) }

    pub fn pref_btn_hit(&self, p: Pt) -> Option<Btn> {
        if let Some(i) = (0..MODULES.len()).find(|&i| self.pref_cell(i).contains(p)) { return Some(Btn::Pref(PBtn::Module(i))); }
        let n = MODULES[self.prefs.module].switches.len();
        (0..n).find(|&i| self.pref_switch(i).contains(p)).map(|i| Btn::Pref(PBtn::Switch(i)))
    }
    pub fn pref_mouse_down(&mut self, p: Pt) {
        if self.prefs.module == DISPLAY && self.pref_slider().inset(-3).contains(p) {
            self.prefs.drag = Some(self.pref_scale_at(p.x));
            self.capture = Some(Capture::PrefSlider);
            return;
        }
        if let Some(b) = self.pref_btn_hit(p) { self.capture = Some(Capture::Press(b)); }
    }
    pub fn pref_slider_drag(&mut self, p: Pt) { self.prefs.drag = Some(self.pref_scale_at(p.x)); }
    pub fn pref_slider_drop(&mut self) { if let Some(v) = self.prefs.drag.take() { self.set_zoom(v); } }
    pub fn pref_btn(&mut self, b: PBtn) {
        match b {
            PBtn::Module(i) => { self.prefs.module = i; self.prefs.drag = None; }
            PBtn::Switch(i) => { if let Some(&(_, a)) = MODULES[self.prefs.module].switches.get(i) { self.act(a); } }
        }
    }

    pub fn pref_draw(&self, p: &mut Painter, c: Rect) {
        let pressed = self.pressed(); // a cell goes white under the mouse, as a matrix cell does
        // The module icons: a sunken scroll view, one cell per module, separators between them.
        let b = self.pref_box();
        p.fill(b, LIGHT);
        p.hline(b.x, b.y, b.w, DARK); p.hline(b.x, b.y + 1, b.w, BLACK);
        p.vline(b.x, b.y, b.h, DARK); p.vline(b.x + 1, b.y, b.h, BLACK);
        p.hline(b.x, b.bottom() - 1, b.w, WHITE); p.vline(b.right() - 1, b.y, b.h, WHITE);
        for i in 0..=MODULES.len() {
            let r = self.pref_cell(i);
            p.vline(r.x, r.y, r.h, DARK); p.vline(r.x + 1, r.y, r.h, BLACK);
            let Some(m) = MODULES.get(i) else { break }; // the matrix closes with one more separator
            let face = rect(r.x + 2, r.y, r.w - 2, r.h);
            let on = i == self.prefs.module || pressed == Some(Btn::Pref(PBtn::Module(i)));
            p.fill(face, if on { WHITE } else { LIGHT });
            p.hline(face.x, face.y, face.w, WHITE); p.vline(face.x, face.y, face.h, WHITE);
            p.icon(m.icon, face.x + (face.w - 48) / 2, face.y + (face.h - 48) / 2, 48);
        }
        // 1.0 kept the scroller of a view that does not scroll: the frame and the empty slot, no
        // knob and no arrows. ponytail: three modules always fit; give it a Scroller if they stop.
        let s = rect(b.x, b.y + 2 + CELL_H, b.w - 2, 21);
        p.hline(s.x, s.y, s.w, DARK); p.hline(s.x, s.y + 1, s.w, BLACK);
        p.dither(rect(s.x + 3, s.y + 3, s.w - 4, 16));

        let pane = self.pref_pane();
        p.hline(c.x, pane.y - 2, c.w, DARK); p.hline(c.x, pane.y - 1, c.w, WHITE);
        let m = &MODULES[self.prefs.module];
        p.text_oblique(FontId::Regular, 16, pane.x + MARGIN + 2, pane.y + 24, &format!("{} Preferences", m.name), BLACK);

        let g = self.pref_group();
        group(p, g, m.group);
        for (i, (label, act)) in m.switches.iter().enumerate() {
            let r = self.pref_switch(i);
            switch(p, rect(r.x, r.y, 15, 15), self.act_state(*act).1);
            p.text_in(FontId::Regular, 12, rect(r.x + 21, r.y, r.w - 21, r.h), Align::Left, label, BLACK);
        }
        if !m.switches.is_empty() {
            if self.prefs.module == EXPERT {
                let path = crate::backend::state::state_path().display().to_string();
                p.text(FontId::Regular, 12, g.x, g.bottom() + 24, "Dot files, the way the Shell sees them.", BLACK);
                p.text(FontId::Regular, 10, g.x, g.bottom() + 44, "Settings are kept in", DARK);
                let t = p.ellipsize_mid(FontId::Regular, 10, &path, g.w);
                p.text(FontId::Regular, 10, g.x, g.bottom() + 58, &t, DARK);
            }
            return;
        }
        // Display: the scale, on a slider between the two factors the Screen allows.
        let sl = self.pref_slider();
        slider(p, sl, (self.pref_scale() - SCALE_MIN) / (SCALE_MAX - SCALE_MIN));
        let base = sl.bottom() + 12;
        p.text(FontId::Regular, 10, sl.x, base, "0.5×", DARK);
        let w = p.text_width(FontId::Regular, 10, "4×");
        p.text(FontId::Regular, 10, sl.right() - w, base, "4×", DARK);
        field(p, self.pref_field(), &format!("{}×", self.pref_scale()));
        p.text(FontId::Regular, 12, g.x, g.bottom() + 24, "Text and icons are drawn again at the new", BLACK);
        p.text(FontId::Regular, 12, g.x, g.bottom() + 40, "size; nothing on the Screen is magnified.", BLACK);
        p.text(FontId::Regular, 10, g.x, g.bottom() + 62, &format!("The Screen is {} × {} pixels at this scale.", self.w, self.h), DARK);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::pt;

    /// The switches are the menu commands, and the slider sets the scale the menu sets.
    #[test]
    fn switches_follow_the_menu_and_the_slider_sets_the_scale() {
        let mut app = App::test_app();
        app.show_win(WinKind::Preferences);
        app.prefs.module = 1;
        assert!(!app.act_state(Act::ShowDock).1);
        let r = app.pref_switch(0);
        app.pref_mouse_down(pt(r.x + 2, r.y + 2));
        app.handle(Ev::MouseUp(pt(r.x + 2, r.y + 2), Button::Left, Mods::default()));
        assert!(app.state.dock_visible, "the switch ran View ▸ Show Dock");
        assert!(app.act_state(Act::ShowDock).1, "and the menu item is checked");

        app.prefs.module = 0;
        let sl = app.pref_slider();
        let mid = pt(sl.x + sl.w / 2, sl.y + 8);
        app.pref_mouse_down(mid);
        assert_eq!(app.zoom, 1.0, "nothing moves until the mouse is let go");
        app.handle(Ev::MouseMove(pt(sl.right(), sl.y + 8)));
        assert_eq!(app.pref_scale(), SCALE_MAX, "the knob follows the pointer");
        app.handle(Ev::MouseUp(pt(sl.right(), sl.y + 8), Button::Left, Mods::default()));
        assert_eq!(app.zoom, SCALE_MAX);
        assert_eq!(app.state.scale, SCALE_MAX, "and it is kept");
        app.pref_mouse_down(pt(sl.x, sl.y + 8));
        app.handle(Ev::MouseUp(pt(sl.x, sl.y + 8), Button::Left, Mods::default()));
        assert_eq!(app.zoom, SCALE_MIN);
    }

    /// Clicking an icon in the row swaps the pane under it.
    #[test]
    fn the_icon_row_picks_the_module() {
        let mut app = App::test_app();
        app.show_win(WinKind::Preferences);
        for i in (0..MODULES.len()).rev() {
            let r = app.pref_cell(i);
            let p = pt(r.x + 30, r.y + 30);
            assert_eq!(app.pref_btn_hit(p), Some(Btn::Pref(PBtn::Module(i))));
            app.pref_btn(PBtn::Module(i));
            assert_eq!(app.prefs.module, i);
        }
        let b = app.pref_cell(MODULES.len());
        assert!(app.pref_btn_hit(pt(b.x + 30, b.y + 30)).is_none(), "past the last module there is no cell");
    }
}
