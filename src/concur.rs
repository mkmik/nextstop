//! Concurrence — outline, slide deck and presenter in one window, after Lighthouse Design's
//! NeXTSTEP application (the outliner Steve Jobs used for his keynotes; the ancestor of Keynote).
//! The outline is a flat list of rows with a level, which is all a one-window outliner needs.
use crate::app::*;
use crate::chrome::*;
use crate::geom::{rect, Pt, Rect};
use crate::paint::*;

const ROW_H: i32 = 18;
const INDENT: i32 = 18;
const FONT: i32 = 12;
const MAX_LEVEL: usize = 4;
const STRIP_H: i32 = 36; // button strip under the body

/// The deck a fresh installation opens with; it also documents the keys.
const SAMPLE: &str = "\
Concurrence
\tOutline, slides and show in one window
\tLighthouse Design, 1992
Outlining
\tTab demotes, Shift-Tab promotes
\tReturn starts a new topic
\tClick a triangle to collapse
Slides
\tEvery top-level topic is a slide
\tIts children are its bullets
\tDrawn in the four NeXT grays
Presenting
\tPress Present, then Space
\tEscape ends the show";

pub struct Row { pub text: String, pub level: usize, pub collapsed: bool }

pub struct Concur {
    pub rows: Vec<Row>,
    pub sel: usize,
    pub caret: usize,
    pub slides: bool,        // Slide view instead of Outline view
    pub show: Option<usize>, // presenting: index into the slide list
    saved: Option<Rect>,     // window rect from before the show
    pub scroll: i32,
}

impl Concur {
    pub fn from_lines(lines: &[String]) -> Concur {
        let src: Vec<String> = if lines.is_empty() { SAMPLE.lines().map(str::to_string).collect() } else { lines.to_vec() };
        let rows: Vec<Row> = src.iter()
            .map(|l| Row { level: l.chars().take_while(|c| *c == '\t').count().min(MAX_LEVEL), text: l.trim_start_matches('\t').to_string(), collapsed: false })
            .collect();
        Concur { rows, sel: 0, caret: 0, slides: false, show: None, saved: None, scroll: 0 }
    }
    pub fn to_lines(&self) -> Vec<String> { self.rows.iter().map(|r| "\t".repeat(r.level) + &r.text).collect() }
}
impl Default for Concur {
    fn default() -> Self { Concur::from_lines(&[]) }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CBtn { Outline, Slide, Present, Save }
const BUTTONS: [(&str, CBtn); 4] = [("Outline", CBtn::Outline), ("Slide", CBtn::Slide), ("Present", CBtn::Present), ("Save", CBtn::Save)];

/// The largest 4:3 slide that fits in `r` with a margin.
fn slide_rect(r: Rect) -> Rect {
    let w = (r.w - r.w / 10).min((r.h - r.h / 10) * 4 / 3).max(8);
    let h = w * 3 / 4;
    rect(r.x + (r.w - w) / 2, r.y + (r.h - h) / 2, w, h)
}
/// Filled 7 px triangle pointing right (an expandable topic); `tri_down` marks an expanded one.
fn tri_right(p: &mut Painter, x: i32, y: i32, w: i32, color: u32) {
    for i in 0..=w / 2 { p.fill(rect(x + i, y + i, 1, w - 2 * i), color); }
}
fn char_byte(s: &str, i: usize) -> usize { s.char_indices().nth(i).map_or(s.len(), |(b, _)| b) }

impl App {
    // ---- geometry ------------------------------------------------------------------------------
    fn c_body(&self) -> Rect { let c = self.content_rect(WinKind::Concurrence); rect(c.x, c.y, c.w, c.h - STRIP_H) }
    fn c_list(&self) -> Rect { let b = self.c_body(); rect(b.x + SCROLL_W, b.y, b.w - SCROLL_W, b.h) }
    fn c_btn(&self, i: usize) -> Rect {
        let c = self.content_rect(WinKind::Concurrence);
        let w = 78;
        rect(c.x + 10 + i as i32 * (w + 8), c.bottom() - STRIP_H + 6, w, BTN_H)
    }
    pub fn c_scroller(&self) -> Scroller {
        let b = self.c_body();
        let total = self.c_visible().len() as i32 * ROW_H + 4;
        Scroller::framed(rect(b.x, b.y, SCROLL_W, b.h), true, total, b.h, self.concur.scroll.clamp(0, (total - b.h).max(0)))
    }

    // ---- outline structure ----------------------------------------------------------------------
    /// Row indices that are not hidden inside a collapsed topic, in order.
    fn c_visible(&self) -> Vec<usize> {
        let rows = &self.concur.rows;
        let mut out = vec![];
        let mut i = 0;
        while i < rows.len() {
            out.push(i);
            let end = if rows[i].collapsed { self.c_end(i) } else { i + 1 };
            i = end;
        }
        out
    }
    /// One past the last descendant of row `i`.
    fn c_end(&self, i: usize) -> usize {
        let rows = &self.concur.rows;
        let lvl = rows[i].level;
        (i + 1..rows.len()).find(|&j| rows[j].level <= lvl).unwrap_or(rows.len())
    }
    fn c_has_kids(&self, i: usize) -> bool { self.concur.rows.get(i + 1).is_some_and(|n| n.level > self.concur.rows[i].level) }
    pub fn c_slides(&self) -> Vec<usize> { (0..self.concur.rows.len()).filter(|&i| self.concur.rows[i].level == 0).collect() }
    fn c_slide_of(&self, row: usize) -> usize { self.c_slides().iter().rposition(|&i| i <= row).unwrap_or(0) }

    fn c_edited(&mut self) {
        self.state.concurrence = self.concur.to_lines();
        self.dirty();
        self.c_reveal();
    }
    /// Scroll so the selected row is inside the list.
    pub fn c_reveal(&mut self) {
        let Some(pos) = self.c_visible().iter().position(|&i| i == self.concur.sel) else { return };
        let (top, h) = (pos as i32 * ROW_H, self.c_list().h);
        if top < self.concur.scroll { self.concur.scroll = top; }
        else if top + ROW_H > self.concur.scroll + h { self.concur.scroll = top + ROW_H - h; }
    }

    // ---- presenting -------------------------------------------------------------------------------
    /// Enter/leave the show: the window takes the whole Screen and loses its chrome.
    pub fn concur_show(&mut self, on: bool) {
        let k = WinKind::Concurrence;
        if on {
            if self.concur.saved.is_some() || self.c_slides().is_empty() { return; }
            self.concur.saved = Some(self.win(k).r);
            self.concur.show = Some(self.c_slide_of(self.concur.sel));
            let (w, h) = (self.w, self.h);
            let win = self.win_mut(k);
            win.r = rect(0, 0, w, h);
            win.chrome = false;
            win.resizable = false;
        } else {
            let Some(r) = self.concur.saved.take() else { return };
            if let Some(s) = self.concur.show.take() {
                if let Some(&i) = self.c_slides().get(s) { self.concur.sel = i; self.concur.caret = 0; }
            }
            let win = self.win_mut(k);
            win.r = r;
            win.chrome = true;
            win.resizable = true;
            self.c_reveal();
        }
        self.make_key(k);
    }
    /// Advance the show; past the last slide it ends, as a keynote should.
    fn c_step(&mut self, d: i32) {
        let n = self.c_slides().len() as i32;
        let next = self.concur.show.unwrap_or(0) as i32 + d;
        if next >= n { self.concur_show(false); } else { self.concur.show = Some(next.max(0) as usize); }
    }
    fn c_goto_slide(&mut self, d: i32) {
        let slides = self.c_slides();
        let cur = self.c_slide_of(self.concur.sel) as i32;
        if let Some(&i) = slides.get((cur + d).clamp(0, slides.len() as i32 - 1) as usize) { self.concur.sel = i; self.concur.caret = 0; }
    }

    // ---- input --------------------------------------------------------------------------------------
    pub fn concur_btn(&mut self, b: CBtn) {
        match b {
            CBtn::Outline => self.concur.slides = false,
            CBtn::Slide => self.concur.slides = true,
            CBtn::Present => self.concur_show(true),
            CBtn::Save => self.concur_save(),
        }
    }
    fn concur_save(&mut self) {
        let text = self.concur.to_lines().join("\n") + "\n";
        let path = (1..).map(|n| crate::icons::join(&self.home, &format!("Presentation-{n}.txt"))).find(|p| !std::path::Path::new(p).exists()).unwrap();
        match std::fs::write(&path, text) {
            Ok(()) => { self.log(format!("saved {path}")); self.fv_refresh(); }
            Err(e) => self.error("Cannot save outline", e.to_string()),
        }
    }
    pub fn concur_btn_hit(&self, p: Pt) -> Option<Btn> {
        if self.concur.show.is_some() { return None; }
        if let Some(i) = (0..BUTTONS.len()).find(|&i| self.c_btn(i).contains(p)) { return Some(Btn::Concur(BUTTONS[i].1)); }
        if self.concur.slides { return None; }
        match self.c_scroller().hit(p)? {
            ScrollHit::ArrowA => Some(Btn::ScrollArrow(ScrollId::Outline, -1)),
            ScrollHit::ArrowB => Some(Btn::ScrollArrow(ScrollId::Outline, 1)),
            _ => None,
        }
    }
    pub fn concur_mouse_down(&mut self, p: Pt) {
        if self.concur.show.is_some() { self.c_step(1); return; }
        if let Some(i) = (0..BUTTONS.len()).find(|&i| self.c_btn(i).contains(p)) { self.capture = Some(Capture::Press(Btn::Concur(BUTTONS[i].1))); return; }
        if self.concur.slides { return; }
        let sc = self.c_scroller();
        if self.scroller_down(ScrollId::Outline, sc, p) { return; }
        let list = self.c_list();
        if !list.contains(p) { return; }
        let vis = self.c_visible();
        let Some(&row) = vis.get(((p.y - list.y + self.concur.scroll) / ROW_H).max(0) as usize) else { return };
        let text_x = list.x + 6 + self.concur.rows[row].level as i32 * INDENT;
        if self.c_has_kids(row) && p.x < text_x + 12 {
            self.concur.rows[row].collapsed = !self.concur.rows[row].collapsed;
            self.concur.sel = row;
            return;
        }
        // caret at the clicked character
        let text: Vec<char> = self.concur.rows[row].text.chars().collect();
        let width = |n: usize| self.fonts.width_px(FontId::Regular, FONT as u32, &text[..n].iter().collect::<String>()) as i32;
        let want = p.x - (text_x + 12);
        self.concur.sel = row;
        self.concur.caret = (0..=text.len()).min_by_key(|&n| (width(n) - want).abs()).unwrap_or(0);
    }

    pub fn concur_key(&mut self, k: Key, mods: Mods) {
        if self.concur.show.is_some() {
            match k {
                Key::Escape | Key::Char('q') => self.concur_show(false),
                Key::Left | Key::Up | Key::PageUp | Key::Backspace => self.c_step(-1),
                _ => self.c_step(1),
            }
            return;
        }
        if self.concur.slides {
            match k {
                Key::Up | Key::Left | Key::PageUp => self.c_goto_slide(-1),
                Key::Down | Key::Right | Key::PageDown | Key::Char(' ') => self.c_goto_slide(1),
                Key::Escape => self.concur.slides = false,
                Key::Enter => self.concur_show(true),
                _ => {}
            }
            return;
        }
        let sel = self.concur.sel.min(self.concur.rows.len() - 1);
        let caret = self.concur.caret.min(self.concur.rows[sel].chars_len());
        self.concur.sel = sel;
        match k {
            Key::Char(c) if !c.is_control() && !mods.ctrl => {
                let b = char_byte(&self.concur.rows[sel].text, caret);
                self.concur.rows[sel].text.insert(b, c);
                self.concur.caret = caret + 1;
            }
            Key::Enter => {
                let b = char_byte(&self.concur.rows[sel].text, caret);
                let rest = self.concur.rows[sel].text.split_off(b);
                let level = self.concur.rows[sel].level;
                self.concur.rows.insert(sel + 1, Row { text: rest, level, collapsed: false });
                self.concur.rows[sel].collapsed = false;
                self.concur.sel = sel + 1;
                self.concur.caret = 0;
            }
            Key::Tab => {
                let end = self.c_end(sel);
                let ok = if mods.shift { self.concur.rows[sel].level > 0 } else { sel > 0 && self.concur.rows[sel].level <= self.concur.rows[sel - 1].level && self.concur.rows[sel].level < MAX_LEVEL };
                if ok { for r in &mut self.concur.rows[sel..end] { r.level = if mods.shift { r.level - 1 } else { r.level + 1 }; } }
            }
            Key::Backspace if caret > 0 => {
                let b = char_byte(&self.concur.rows[sel].text, caret - 1);
                self.concur.rows[sel].text.remove(b);
                self.concur.caret = caret - 1;
            }
            // join with the row above, unless that would orphan this topic's children
            Key::Backspace if sel > 0 && !self.c_has_kids(sel) => {
                let text = self.concur.rows.remove(sel).text;
                self.concur.sel = sel - 1;
                self.concur.caret = self.concur.rows[sel - 1].chars_len();
                self.concur.rows[sel - 1].text.push_str(&text);
            }
            Key::Delete if caret < self.concur.rows[sel].chars_len() => {
                let b = char_byte(&self.concur.rows[sel].text, caret);
                self.concur.rows[sel].text.remove(b);
            }
            Key::Left => self.concur.caret = caret.saturating_sub(1),
            Key::Right => self.concur.caret = (caret + 1).min(self.concur.rows[sel].chars_len()),
            Key::Home => self.concur.caret = 0,
            Key::End => self.concur.caret = self.concur.rows[sel].chars_len(),
            Key::Up | Key::Down => {
                let vis = self.c_visible();
                let pos = vis.iter().position(|&i| i == sel).unwrap_or(0) as i32 + if k == Key::Up { -1 } else { 1 };
                if let Some(&i) = vis.get(pos.clamp(0, vis.len() as i32 - 1) as usize) { self.concur.sel = i; self.concur.caret = self.concur.caret.min(self.concur.rows[i].chars_len()); }
            }
            Key::Escape => self.concur.slides = true,
            _ => {}
        }
        self.c_edited();
    }

    // ---- drawing -----------------------------------------------------------------------------------
    pub fn concur_draw(&self, p: &mut Painter, c: Rect) {
        if let Some(s) = self.concur.show {
            p.fill(c, BLACK);
            self.c_draw_slide(p, slide_rect(c), s);
            return;
        }
        let body = self.c_body();
        if self.concur.slides {
            p.fill(body, DARK); // the light table around the slide
            self.c_draw_slide(p, slide_rect(body), self.c_slide_of(self.concur.sel));
        } else {
            self.c_draw_outline(p, body);
        }
        let pressed = self.pressed();
        for (i, (label, b)) in BUTTONS.iter().enumerate() {
            let active = (*b == CBtn::Outline && !self.concur.slides) || (*b == CBtn::Slide && self.concur.slides);
            button(p, self.c_btn(i), label, active || pressed == Some(Btn::Concur(*b)), false);
        }
    }

    fn c_draw_outline(&self, p: &mut Painter, body: Rect) {
        let sc = self.c_scroller();
        sc.draw(p, self.pressed().and_then(|b| match b {
            Btn::ScrollArrow(ScrollId::Outline, d) => Some(if d < 0 { ScrollHit::ArrowA } else { ScrollHit::ArrowB }),
            _ => None,
        }));
        let list = self.c_list();
        p.fill(list, WHITE);
        p.push_clip(list);
        let key = self.key == Some(WinKind::Concurrence);
        for (pos, &row) in self.c_visible().iter().enumerate() {
            let r = rect(list.x, list.y + pos as i32 * ROW_H - sc.pos, list.w, ROW_H);
            if r.bottom() < list.y || r.y > list.bottom() { continue; }
            let it = &self.concur.rows[row];
            let x = list.x + 6 + it.level as i32 * INDENT;
            if row == self.concur.sel { p.fill(r, LIGHT); }
            let base = p.baseline(FontId::Regular, FONT, r.y, ROW_H);
            if self.c_has_kids(row) {
                if it.collapsed { tri_right(p, x + 1, r.y + 6, 7, BLACK) } else { tri_down(p, x, r.y + 7, 7, BLACK) }
            } else {
                p.fill(rect(x + 2, r.y + 8, 3, 3), BLACK);
            }
            let font = if it.level == 0 { FontId::Bold } else { FontId::Regular };
            p.text(font, FONT, x + 12, base, &it.text, BLACK);
            if row == self.concur.sel && key {
                let w = p.text_width(font, FONT, &it.text.chars().take(self.concur.caret).collect::<String>());
                p.fill(rect(x + 12 + w, r.y + 2, 1, ROW_H - 4), BLACK);
            }
        }
        p.pop_clip();
        p.vline(list.x, body.y, body.h, BLACK);
    }

    /// One slide: title, rule and bullets, scaled to `r` so the window and the show share this code.
    fn c_draw_slide(&self, p: &mut Painter, r: Rect, slide: usize) {
        p.fill(r, WHITE);
        p.outline(r, BLACK);
        let slides = self.c_slides();
        let Some(&start) = slides.get(slide) else { return };
        let pad = r.w / 14;
        let ts = (r.h / 9).clamp(11, 64);
        let bs = (r.h / 16).clamp(9, 40);
        let rows = &self.concur.rows;
        p.text(FontId::Bold, ts, r.x + pad, r.y + pad + ts, &p.ellipsize(FontId::Bold, ts, &rows[start].text, r.w - 2 * pad), BLACK);
        let rule = r.y + pad + ts + ts / 3;
        p.fill(rect(r.x + pad, rule, r.w - 2 * pad, (ts / 16).max(1)), BLACK);
        let mut y = rule + bs + bs / 2;
        // ponytail: one line per bullet, ellipsized — wrap only if real decks need it
        for it in &rows[start + 1..self.c_end(start)] {
            if y > r.bottom() - pad { break; }
            let lvl = it.level.max(1) as i32;
            let x = r.x + pad + (lvl - 1) * bs * 3 / 2;
            let size = if lvl == 1 { bs } else { bs * 4 / 5 };
            let m = (size / 4).max(2);
            if lvl == 1 { p.fill(rect(x, y - size / 2 - m / 2, m, m), BLACK) } else { p.fill(rect(x, y - size / 2, m + m / 2, (m / 2).max(1)), BLACK) }
            let tx = x + bs;
            p.text(FontId::Regular, size, tx, y, &p.ellipsize(FontId::Regular, size, &it.text, r.right() - pad - tx), BLACK);
            y += size * 3 / 2;
        }
        let n = format!("{} / {}", slide + 1, slides.len());
        let fs = (r.h / 32).clamp(8, 18);
        let w = p.text_width(FontId::Regular, fs, &n);
        p.text(FontId::Regular, fs, r.right() - pad / 2 - w, r.bottom() - pad / 3, &n, DARK);
    }
}

impl Row {
    fn chars_len(&self) -> usize { self.text.chars().count() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut a = App::test_app();
        a.show_win(WinKind::Concurrence);
        a
    }
    fn keys(a: &mut App, ks: &[Key]) { for k in ks { a.concur_key(*k, Mods::default()); } }
    fn typed(a: &mut App, s: &str) { for c in s.chars() { a.concur_key(Key::Char(c), Mods::default()); } }

    #[test]
    fn sample_deck_parses_into_slides_with_bullets() {
        let a = app();
        assert_eq!(a.c_slides().len(), 4);
        assert_eq!(a.concur.rows[1].level, 1);
        assert_eq!(a.concur.rows[1].text, "Outline, slides and show in one window");
        assert_eq!(a.c_slide_of(5), 1);
    }

    #[test]
    fn return_splits_a_topic_and_tab_moves_the_whole_subtree() {
        let mut a = app();
        a.concur.rows.truncate(3); // "Concurrence" + two bullets
        a.concur.sel = 0;
        a.concur.caret = 4;
        keys(&mut a, &[Key::Enter]);
        assert_eq!((a.concur.rows[0].text.as_str(), a.concur.rows[1].text.as_str()), ("Conc", "urrence"));
        assert_eq!(a.concur.sel, 1);
        // demoting the new level-0 row must take its two following bullets with it
        keys(&mut a, &[Key::Tab]);
        assert_eq!(a.concur.rows.iter().map(|r| r.level).collect::<Vec<_>>(), vec![0, 1, 2, 2]);
        assert_eq!(a.c_slides().len(), 1);
        a.concur_key(Key::Tab, Mods { shift: true, ..Mods::default() });
        assert_eq!(a.concur.rows.iter().map(|r| r.level).collect::<Vec<_>>(), vec![0, 0, 1, 1]);
    }

    #[test]
    fn typing_and_backspace_edit_at_the_caret_then_join_rows() {
        let mut a = app();
        a.concur.rows.truncate(2);
        a.concur.sel = 1;
        a.concur.caret = 0;
        typed(&mut a, "Ok");
        assert!(a.concur.rows[1].text.starts_with("OkOutline, slides"));
        keys(&mut a, &[Key::Backspace, Key::Backspace]);
        assert!(a.concur.rows[1].text.starts_with("Outline, slides"));
        keys(&mut a, &[Key::Backspace]); // caret 0 in a childless row: join with the one above
        assert_eq!(a.concur.rows.len(), 1);
        assert!(a.concur.rows[0].text.starts_with("ConcurrenceOutline, slides"));
        assert_eq!(a.state.concurrence.len(), 1, "edits are persisted");
    }

    #[test]
    fn collapsed_topics_hide_their_children_and_the_selection_skips_them() {
        let mut a = app();
        a.concur.rows[0].collapsed = true;
        assert_eq!(a.c_visible()[1], 3, "the second visible row is the next slide");
        a.concur.sel = 0;
        keys(&mut a, &[Key::Down]);
        assert_eq!(a.concur.sel, 3);
    }

    #[test]
    fn presenting_takes_the_screen_and_the_last_slide_ends_the_show() {
        let mut a = app();
        let before = a.win(WinKind::Concurrence).r;
        a.concur.sel = 4; // inside the second slide
        a.concur_btn(CBtn::Present);
        assert_eq!(a.concur.show, Some(1));
        assert_eq!(a.win(WinKind::Concurrence).r, rect(0, 0, a.w, a.h));
        assert!(!a.win(WinKind::Concurrence).chrome);
        for _ in 0..3 { a.concur_key(Key::Char(' '), Mods::default()); } // 2 → 3 → 4 → end
        assert_eq!(a.concur.show, None);
        assert_eq!(a.win(WinKind::Concurrence).r, before, "the window comes back where it was");
        assert!(a.win(WinKind::Concurrence).chrome);
        assert_eq!(a.concur.sel, *a.c_slides().last().unwrap(), "the outline lands on the last slide shown");
    }

    #[test]
    fn closing_while_presenting_restores_the_window() {
        let mut a = app();
        let before = a.win(WinKind::Concurrence).r;
        a.concur_btn(CBtn::Present);
        a.close_win(WinKind::Concurrence);
        assert_eq!(a.concur.show, None);
        assert_eq!(a.win(WinKind::Concurrence).r, before);
        assert!(a.win(WinKind::Concurrence).chrome);
    }
}
