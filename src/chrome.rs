//! Window chrome, scrollers, menus, glyphs — geometry taken from NeXTSTEP 1.0 screenshots (see docs/DECISIONS.md).
use crate::geom::{rect, Pt, Rect};
use crate::paint::*;

pub const TITLE_H: i32 = 22;   // highlight row + 19 fill rows + dark row + black row
pub const RESIZE_H: i32 = 8;   // dark row + white row + 6 face rows
pub const MENU_ITEM_H: i32 = 20;
pub const MENU_TITLE_H: i32 = 22;
pub const SCROLL_W: i32 = 18;  // 2 px border + 16 px knob/buttons
pub const BTN_H: i32 = 24;

// ---- glyphs -------------------------------------------------------------------------------

/// Filled right-pointing triangle, `h` px tall, left edge at (x, y).
pub fn tri_right(p: &mut Painter, x: i32, y: i32, h: i32, color: u32) {
    let half = h / 2;
    for i in 0..=half { p.fill(rect(x + i, y + i, 1, h - 2 * i), color); }
}
pub fn tri_left(p: &mut Painter, x: i32, y: i32, h: i32, color: u32) {
    let half = h / 2;
    for i in 0..=half { p.fill(rect(x + half - i, y + i, 1, h - 2 * i), color); }
}
pub fn tri_up(p: &mut Painter, x: i32, y: i32, w: i32, color: u32) {
    let half = w / 2;
    for i in 0..=half { p.fill(rect(x + half - i, y + i, 2 * i + 1, 1), color); }
}
pub fn tri_down(p: &mut Painter, x: i32, y: i32, w: i32, color: u32) {
    let half = w / 2;
    for i in 0..=half { p.fill(rect(x + i, y + i, w - 2 * i, 1), color); }
}
/// Hollow right-pointing triangle (the 1.0 submenu / folder marker), 7 px tall, 4 px wide.
pub fn tri_hollow_right(p: &mut Painter, x: i32, y: i32, color: u32) {
    p.fill(rect(x, y, 1, 7), color);
    for i in 0..3 { p.fill(rect(x + 1 + i, y + 1 + i, 1, 1), color); p.fill(rect(x + 1 + i, y + 5 - i, 1, 1), color); }
    p.fill(rect(x + 3, y + 3, 1, 1), color);
}
/// 14×14 title-bar button glyphs: a tiny window (miniaturize) and a 2 px X (close).
pub fn glyph_mini(p: &mut Painter, r: Rect) { p.outline(rect(r.x + 2, r.y + 2, 10, 10), BLACK); p.fill(rect(r.x + 2, r.y + 2, 10, 3), BLACK); }
pub fn glyph_close(p: &mut Painter, r: Rect) {
    for i in 0..8 {
        p.fill(rect(r.x + 3 + i, r.y + 3 + i, 1, 1), BLACK); p.fill(rect(r.x + 4 + i, r.y + 3 + i, 1, 1), if i == 7 { DARK } else { BLACK });
        p.fill(rect(r.x + 10 - i, r.y + 3 + i, 1, 1), BLACK); p.fill(rect(r.x + 9 - i, r.y + 3 + i, 1, 1), if i == 7 { DARK } else { BLACK });
    }
}
/// Return-key glyph on default buttons.
pub fn glyph_return(p: &mut Painter, x: i32, y: i32) {
    p.fill(rect(x + 8, y, 2, 6), BLACK); p.fill(rect(x + 2, y + 5, 8, 1), BLACK);
    for i in 0..3 { p.fill(rect(x + 2 + i, y + 5 - 3 + i, 1, 1), BLACK); p.fill(rect(x + 2 + i, y + 5 + 3 - i, 1, 1), BLACK); }
}
/// Sunken 6×6 "dimple" centred at (cx, cy).
pub fn dimple(p: &mut Painter, cx: i32, cy: i32) {
    let r = rect(cx - 3, cy - 3, 6, 6);
    p.fill(r, LIGHT);
    p.hline(r.x, r.y, 6, BLACK); p.vline(r.x, r.y, 6, BLACK);
    p.hline(r.x + 1, r.y + 1, 4, DARK); p.vline(r.x + 1, r.y + 1, 4, DARK);
    p.hline(r.x + 1, r.bottom() - 1, 5, WHITE); p.vline(r.right() - 1, r.y + 1, 5, WHITE);
}

/// Raised push button with centred label; `default` adds the return glyph.
pub fn button(p: &mut Painter, r: Rect, label: &str, pressed: bool, default: bool) {
    if pressed { p.pressed(r) } else { p.raised(r) }
    let lr = if default { rect(r.x, r.y, r.w - 14, r.h) } else { r };
    p.text_in(FontId::Regular, 12, lr, Align::Center, label, BLACK);
    if default { glyph_return(p, r.right() - 18, r.y + r.h / 2 - 4); }
}

/// Thick Dock-style tile bevel (2 px white top/left, dark + 2 px black bottom/right).
pub fn tile_bevel(p: &mut Painter, r: Rect) {
    p.fill(r, LIGHT);
    p.fill(rect(r.x, r.y, r.w, 2), WHITE); p.fill(rect(r.x, r.y, 2, r.h), WHITE);
    p.fill(rect(r.x + 2, r.bottom() - 3, r.w - 2, 1), DARK); p.fill(rect(r.right() - 3, r.y + 2, 1, r.h - 2), DARK);
    p.fill(rect(r.x + 2, r.bottom() - 2, r.w - 2, 2), BLACK); p.fill(rect(r.right() - 2, r.y + 2, 2, r.h - 2), BLACK);
}

// ---- windows -------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum WinKind { FileViewer, Inspector, Console, Info, Recycler, Alert }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WinPart { Title, MiniBtn, CloseBtn, Resize(i8), Content }

pub struct Win {
    pub kind: WinKind,
    pub r: Rect,
    pub title: String,
    pub min_w: i32, pub min_h: i32,
    pub resizable: bool, pub mini_btn: bool, pub close_btn: bool,
    pub visible: bool,
    pub mini: Option<usize>,
    pub icon: &'static str,
    pub z: u32,
}

impl Win {
    pub fn new(kind: WinKind, r: Rect, title: &str, icon: &'static str) -> Win {
        Win { kind, r, title: title.into(), min_w: 120, min_h: 60, resizable: true, mini_btn: true, close_btn: true, visible: false, mini: None, icon, z: 0 }
    }
    pub fn title_bar(&self) -> Rect { rect(self.r.x, self.r.y, self.r.w, TITLE_H) }
    pub fn content(&self) -> Rect { rect(self.r.x, self.r.y + TITLE_H, self.r.w, self.r.h - TITLE_H - if self.resizable { RESIZE_H } else { 0 }) }
    pub fn resize_bar(&self) -> Option<Rect> { self.resizable.then(|| rect(self.r.x, self.r.bottom() - RESIZE_H, self.r.w, RESIZE_H)) }
    pub fn mini_rect(&self) -> Option<Rect> { self.mini_btn.then(|| rect(self.r.x + 3, self.r.y + 3, 14, 14)) }
    pub fn close_rect(&self) -> Option<Rect> { self.close_btn.then(|| rect(self.r.right() - 17, self.r.y + 3, 14, 14)) }
    pub fn shown(&self) -> bool { self.visible && self.mini.is_none() }

    pub fn hit(&self, p: Pt) -> Option<WinPart> {
        if !self.r.contains(p) { return None; }
        if self.mini_rect().is_some_and(|r| r.contains(p)) { return Some(WinPart::MiniBtn); }
        if self.close_rect().is_some_and(|r| r.contains(p)) { return Some(WinPart::CloseBtn); }
        if self.title_bar().contains(p) { return Some(WinPart::Title); }
        if let Some(rb) = self.resize_bar() {
            if rb.contains(p) { return Some(WinPart::Resize(if p.x < rb.x + 29 { -1 } else if p.x >= rb.right() - 29 { 1 } else { 0 })); }
        }
        Some(WinPart::Content)
    }

    /// §7.2: the title bar must stay at least 20 px inside the Screen on every side.
    pub fn clamp(&mut self, sw: i32, sh: i32) {
        self.r.x = self.r.x.max(20 - self.r.w).min(sw - 20);
        self.r.y = self.r.y.max(20 - TITLE_H).min(sh - 20);
    }

    pub fn draw_chrome(&self, p: &mut Painter, key: bool, pressed: Option<WinPart>) {
        let r = self.r;
        p.outline(rect(r.x - 1, r.y - 1, r.w + 2, r.h + 2), BLACK);
        // title bar: highlight row/column, fill, dark row + column, black separator
        let (fill, hl, fg) = if key { (BLACK, LIGHT, WHITE) } else { (LIGHT, WHITE, BLACK) };
        let tb = self.title_bar();
        p.fill(rect(tb.x, tb.y, tb.w, 20), fill);
        p.hline(tb.x, tb.y, tb.w, hl);
        p.vline(tb.x, tb.y, 20, hl);
        p.vline(tb.right() - 1, tb.y, 20, DARK);
        p.hline(tb.x, tb.y + 20, tb.w, DARK);
        p.hline(tb.x, tb.y + 21, tb.w, BLACK);
        let title = p.ellipsize(FontId::Bold, 12, &self.title, tb.w - 44);
        p.text_in(FontId::Bold, 12, rect(tb.x + 20, tb.y, tb.w - 40, 20), Align::Center, &title, fg);
        if let Some(b) = self.mini_rect() { if pressed == Some(WinPart::MiniBtn) { p.pressed(b) } else { p.raised(b) } glyph_mini(p, b); }
        if let Some(b) = self.close_rect() { if pressed == Some(WinPart::CloseBtn) { p.pressed(b) } else { p.raised(b) } glyph_close(p, b); }
        p.fill(self.content(), LIGHT);
        if let Some(rb) = self.resize_bar() {
            p.hline(rb.x, rb.y, rb.w, DARK);
            p.hline(rb.x, rb.y + 1, rb.w, WHITE);
            p.fill(rect(rb.x, rb.y + 2, rb.w, 6), LIGHT);
            for x in [rb.x + 28, rb.right() - 30] { p.vline(x, rb.y + 2, 6, DARK); p.vline(x + 1, rb.y + 2, 6, WHITE); }
        }
    }
}

/// Miniwindow tile (§7.2): Dock-style tile with a black title strip on top.
pub fn miniwindow_rect(slot: usize, sh: i32) -> Rect { rect(64 * (slot as i32 + 1), sh - 64, 64, 64) }
pub fn draw_miniwindow(p: &mut Painter, r: Rect, icon: &str, title: &str) {
    tile_bevel(p, r);
    let strip = rect(r.x + 2, r.y + 2, r.w - 5, 11);
    p.fill(strip, BLACK);
    let t = p.ellipsize_mid(FontId::Regular, 10, title, strip.w - 4);
    p.text_in(FontId::Regular, 10, strip, Align::Center, &t, WHITE);
    p.icon(icon, r.x + 8, r.y + 14, 44);
}

// ---- scroller (§7.10, 1.0 style: dithered track, arrows at the bottom) ---------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScrollHit { ArrowA, ArrowB, PageBack, PageFwd, Knob }

#[derive(Clone, Copy, Debug)]
pub struct Scroller { pub r: Rect, pub vertical: bool, pub total: i32, pub visible: i32, pub pos: i32 }

impl Scroller {
    pub fn max_pos(&self) -> i32 { (self.total - self.visible).max(0) }
    pub fn fits(&self) -> bool { self.total <= self.visible }
    fn len(&self) -> i32 { if self.vertical { self.r.h } else { self.r.w } }
    fn track_len(&self) -> i32 { self.len() - 32 - 2 }
    pub fn arrow_a(&self) -> Rect { if self.vertical { rect(self.r.x + 2, self.r.bottom() - 32, 16, 16) } else { rect(self.r.right() - 32, self.r.y + 2, 16, 16) } }
    pub fn arrow_b(&self) -> Rect { if self.vertical { rect(self.r.x + 2, self.r.bottom() - 16, 16, 16) } else { rect(self.r.right() - 16, self.r.y + 2, 16, 16) } }
    fn knob_len(&self) -> i32 {
        let tl = self.track_len();
        ((tl as i64 * self.visible as i64) / self.total.max(1) as i64).max(16).min(tl as i64) as i32
    }
    pub fn knob(&self) -> Option<Rect> {
        if self.fits() { return None; }
        let tl = self.track_len();
        let kl = self.knob_len();
        let off = if self.max_pos() == 0 { 0 } else { ((tl - kl) as i64 * self.pos.clamp(0, self.max_pos()) as i64 / self.max_pos() as i64) as i32 };
        Some(if self.vertical { rect(self.r.x + 2, self.r.y + 2 + off, 16, kl) } else { rect(self.r.x + 2 + off, self.r.y + 2, kl, 16) })
    }
    pub fn drag_pos(&self, start_pos: i32, delta: i32) -> i32 {
        let free = self.track_len() - self.knob_len();
        if free <= 0 { return start_pos; }
        (start_pos + (delta as i64 * self.max_pos() as i64 / free as i64) as i32).clamp(0, self.max_pos())
    }
    pub fn hit(&self, p: Pt) -> Option<ScrollHit> {
        if !self.r.contains(p) { return None; }
        if self.arrow_a().contains(p) { return Some(ScrollHit::ArrowA); }
        if self.arrow_b().contains(p) { return Some(ScrollHit::ArrowB); }
        let k = self.knob()?;
        if k.contains(p) { return Some(ScrollHit::Knob); }
        let before = if self.vertical { p.y < k.y } else { p.x < k.x };
        Some(if before { ScrollHit::PageBack } else { ScrollHit::PageFwd })
    }
    pub fn draw(&self, p: &mut Painter, pressed: Option<ScrollHit>) {
        let track = if self.vertical { rect(self.r.x, self.r.y, self.r.w, self.r.h - 32) } else { rect(self.r.x, self.r.y, self.r.w - 32, self.r.h) };
        p.dither(track);
        p.hline(track.x, track.y, track.w, DARK); p.hline(track.x, track.y + 1, track.w, BLACK);
        p.vline(track.x, track.y, track.h, DARK); p.vline(track.x + 1, track.y, track.h, BLACK);
        let disabled = self.fits();
        let col = if disabled { DARK } else { BLACK };
        for (r, hit, which) in [(self.arrow_a(), ScrollHit::ArrowA, 0), (self.arrow_b(), ScrollHit::ArrowB, 1)] {
            if pressed == Some(hit) && !disabled { p.pressed(r) } else { p.raised(r) }
            match (self.vertical, which) {
                (true, 0) => tri_up(p, r.x + 4, r.y + 4, 7, col),
                (true, _) => tri_down(p, r.x + 4, r.y + 5, 7, col),
                (false, 0) => tri_left(p, r.x + 4, r.y + 4, 7, col),
                (false, _) => tri_right(p, r.x + 5, r.y + 4, 7, col),
            }
        }
        if let Some(k) = self.knob() {
            p.raised(k);
            let c = k.center();
            dimple(p, c.x, c.y);
        }
    }
}

// ---- menus (§7.3) ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Act {
    None, Disabled, InfoPanel, Open, NewFolder, Duplicate, Destroy, EmptyRecycler, Copy, Paste, SelectAll, CheckDisks,
    ViewBrowser, Scale1, Scale2, ShowHidden, Backdrop, ShowDock, ShowMiniwindows, ShowRecycler, Inspector, ConsoleWin, FileViewerWin, RecyclerWin, ArrangeFront, Miniaturize, CloseWin, Hide, Quit,
}

pub struct ItemDef { pub label: &'static str, pub key: Option<char>, pub sub: Option<&'static [ItemDef]>, pub act: Act }
const fn item(label: &'static str, key: Option<char>, act: Act) -> ItemDef { ItemDef { label, key, sub: None, act } }
const fn sub(label: &'static str, sub: &'static [ItemDef]) -> ItemDef { ItemDef { label, key: None, sub: Some(sub), act: Act::None } }

pub static SCALE_MENU: [ItemDef; 2] = [item("1×", None, Act::Scale1), item("2×", None, Act::Scale2)];
pub static INFO_MENU: [ItemDef; 3] = [item("Info Panel…", None, Act::InfoPanel), item("Preferences…", None, Act::Disabled), item("Help…", None, Act::Disabled)];
pub static FILE_MENU: [ItemDef; 7] = [
    item("Open", Some('o'), Act::Open), item("Open as Folder", Some('O'), Act::Disabled), item("New Folder", Some('n'), Act::NewFolder),
    item("Duplicate", Some('d'), Act::Duplicate), item("Compress", None, Act::Disabled), item("Destroy", Some('r'), Act::Destroy), item("Empty Recycler", None, Act::EmptyRecycler),
];
pub static EDIT_MENU: [ItemDef; 4] = [item("Cut", Some('x'), Act::Disabled), item("Copy", Some('c'), Act::Copy), item("Paste", Some('v'), Act::Paste), item("Select All", Some('a'), Act::SelectAll)];
pub static DISK_MENU: [ItemDef; 2] = [item("Check for Disks", None, Act::CheckDisks), item("Eject", None, Act::Disabled)];
pub static VIEW_MENU: [ItemDef; 9] = [
    item("Browser", None, Act::ViewBrowser), item("Icon", None, Act::Disabled), item("Listing", None, Act::Disabled), sub("Scale", &SCALE_MENU),
    item("Show Hidden Files", None, Act::ShowHidden), item("Show Dock", None, Act::ShowDock), item("Show Miniwindows", None, Act::ShowMiniwindows),
    item("Show Recycler Tile", None, Act::ShowRecycler), item("Screen Backdrop", None, Act::Backdrop),
];
pub static TOOLS_MENU: [ItemDef; 4] = [item("Inspector…", Some('i'), Act::Inspector), item("Finder…", None, Act::Disabled), item("Processes…", None, Act::Disabled), item("Console…", None, Act::ConsoleWin)];
pub static WINDOWS_MENU: [ItemDef; 5] = [item("File Viewer", None, Act::FileViewerWin), item("Recycler", None, Act::RecyclerWin), item("Arrange in Front", None, Act::ArrangeFront), item("Miniaturize Window", Some('m'), Act::Miniaturize), item("Close Window", Some('w'), Act::CloseWin)];
pub static SERVICES_MENU: [ItemDef; 1] = [item("No Services Available", None, Act::Disabled)];
pub static MAIN_MENU: [ItemDef; 10] = [
    sub("Info", &INFO_MENU), sub("File", &FILE_MENU), sub("Edit", &EDIT_MENU), sub("Disk", &DISK_MENU), sub("View", &VIEW_MENU),
    sub("Tools", &TOOLS_MENU), sub("Windows", &WINDOWS_MENU), sub("Services", &SERVICES_MENU),
    item("Hide", Some('h'), Act::Hide), item("Quit", Some('q'), Act::Quit),
];

pub fn resolve_path(path: &[String]) -> Option<&'static [ItemDef]> {
    let mut items: &'static [ItemDef] = &MAIN_MENU;
    for label in path { items = items.iter().find(|i| i.label == label)?.sub?; }
    Some(items)
}
pub fn find_key(items: &'static [ItemDef], c: char) -> Option<&'static ItemDef> {
    items.iter().find_map(|i| if let Some(s) = i.sub { find_key(s, c) } else if i.key == Some(c) { Some(i) } else { None })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuKind { Main, Popup, Sub { parent: usize, item: usize }, Torn }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuPart { Title, Close, Item(usize) }

pub struct MenuInst {
    pub id: u64,
    pub title: String,
    pub items: &'static [ItemDef],
    pub path: Vec<String>,
    pub pos: Pt,
    pub w: i32,
    pub kind: MenuKind,
    pub open_item: Option<usize>,
}

impl MenuInst {
    /// Widest of title and items, plus padding for the key letter or ▷ (1.0 menus are as narrow as their contents).
    pub fn width(fonts: &Fonts, items: &[ItemDef], title: &str) -> i32 {
        let mut w = fonts.width_px(FontId::Bold, 12, title) as i32 + 10;
        for it in items { w = w.max(fonts.width_px(FontId::Regular, 12, it.label) as i32 + 6 + 30); }
        w.max(93)
    }
    pub fn height(&self) -> i32 { MENU_TITLE_H + self.items.len() as i32 * MENU_ITEM_H }
    /// Face rectangle (the black shadow column/row lies just outside it).
    pub fn rect(&self) -> Rect { rect(self.pos.x, self.pos.y, self.w, self.height()) }
    pub fn title_rect(&self) -> Rect { rect(self.pos.x, self.pos.y, self.w, MENU_TITLE_H) }
    pub fn close_rect(&self) -> Option<Rect> { (self.kind == MenuKind::Torn).then(|| rect(self.pos.x + self.w - 17, self.pos.y + 3, 14, 14)) }
    pub fn item_rect(&self, idx: usize) -> Rect { rect(self.pos.x, self.pos.y + MENU_TITLE_H + idx as i32 * MENU_ITEM_H, self.w, MENU_ITEM_H) }
    pub fn hit(&self, p: Pt) -> Option<MenuPart> {
        if !self.rect().contains(p) { return None; }
        if self.close_rect().is_some_and(|r| r.contains(p)) { return Some(MenuPart::Close); }
        if self.title_rect().contains(p) { return Some(MenuPart::Title); }
        (0..self.items.len()).find(|&i| self.item_rect(i).contains(p)).map(MenuPart::Item)
    }

    pub fn draw(&self, p: &mut Painter, hi: Option<usize>, close_pressed: bool, state: &dyn Fn(&ItemDef) -> (bool, bool)) {
        let r = self.rect();
        // black shadow along the right and bottom of the whole menu
        p.vline(r.right(), r.y, r.h + 1, BLACK);
        p.hline(r.x, r.bottom(), r.w + 1, BLACK);
        // title: raised black cell (white highlight, dark shadow) + black shadow row
        let tr = self.title_rect();
        p.fill(rect(tr.x, tr.y, tr.w, 21), BLACK);
        p.hline(tr.x, tr.y, tr.w, WHITE); p.vline(tr.x, tr.y, 21, WHITE);
        p.hline(tr.x, tr.y + 20, tr.w, DARK); p.vline(tr.right() - 1, tr.y, 21, DARK);
        p.hline(tr.x, tr.y + 21, tr.w, BLACK);
        let tw = tr.w - 6 - if self.kind == MenuKind::Torn { 16 } else { 0 };
        p.text_in(FontId::Bold, 12, rect(tr.x + 5, tr.y, tw, 20), Align::Left, &p.ellipsize(FontId::Bold, 12, &self.title, tw), WHITE);
        if let Some(c) = self.close_rect() { if close_pressed { p.pressed(c) } else { p.raised(c) } glyph_close(p, c); }
        for (i, it) in self.items.iter().enumerate() {
            let ir = self.item_rect(i);
            let (disabled, checked) = state(it);
            let inverted = hi == Some(i) || self.open_item == Some(i);
            let face = rect(ir.x, ir.y, ir.w, 19);
            p.fill(face, if inverted { BLACK } else { LIGHT });
            p.hline(face.x, face.y, face.w, WHITE); p.vline(face.x, face.y, 19, WHITE);
            p.hline(face.x, face.bottom() - 1, face.w, DARK); p.vline(face.right() - 1, face.y, 19, DARK);
            p.hline(ir.x, ir.bottom() - 1, ir.w, BLACK);
            let fg = if inverted { WHITE } else if disabled { DARK } else { BLACK };
            if checked { p.fill(rect(ir.x + 1, ir.y + 7, 4, 4), fg); }
            p.text_in(FontId::Regular, 12, rect(ir.x + 6, ir.y, ir.w - 34, 19), Align::Left, it.label, fg);
            if it.sub.is_some() { tri_hollow_right(p, ir.right() - 11, ir.y + 6, fg); }
            else if let Some(k) = it.key {
                let kw = p.text_width(FontId::Regular, 12, &k.to_string());
                p.text_in(FontId::Regular, 12, rect(ir.right() - 6 - kw, ir.y, kw, 19), Align::Left, &k.to_string(), fg);
            }
        }
    }
}
