//! Window chrome, scrollers, menus, glyphs (§7.2, §7.3, §7.10).
use crate::geom::{rect, Pt, Rect};
use crate::paint::*;

pub const TITLE_H: i32 = 22;
pub const RESIZE_H: i32 = 8;
pub const MENU_ITEM_H: i32 = 20;

// ---- glyphs -------------------------------------------------------------------------------

/// Right-pointing triangle, `h` px tall (NeXT ▸), left edge at (x, y).
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
/// Our 8×8 "command" glyph: hollow square with a dot.
pub fn cmd_glyph(p: &mut Painter, x: i32, y: i32, color: u32) {
    p.outline(rect(x, y, 8, 8), color);
    p.fill(rect(x + 3, y + 3, 2, 2), color);
}
/// 14×14 title-bar button glyphs.
pub fn glyph_mini(p: &mut Painter, r: Rect) { p.fill(rect(r.x + 4, r.y + 6, 6, 2), BLACK); }
pub fn glyph_close(p: &mut Painter, r: Rect) {
    for i in 0..8 { p.fill(rect(r.x + 3 + i, r.y + 3 + i, 1, 1), BLACK); p.fill(rect(r.x + 10 - i, r.y + 3 + i, 1, 1), BLACK); }
}
/// Small return-key glyph used on default alert buttons.
pub fn glyph_return(p: &mut Painter, x: i32, y: i32) {
    p.fill(rect(x + 8, y, 1, 6), BLACK); p.fill(rect(x + 2, y + 5, 7, 1), BLACK);
    for i in 0..3 { p.fill(rect(x + 2 + i, y + 5 - 3 + i, 1, 1), BLACK); p.fill(rect(x + 2 + i, y + 5 + 3 - i, 1, 1), BLACK); }
}

/// Raised push button with centered label.
pub fn button(p: &mut Painter, r: Rect, label: &str, pressed: bool, default: bool) {
    if pressed { p.pressed(r) } else { p.raised(r) }
    if default { p.outline(r.inset(1), BLACK); }
    let lr = if default { rect(r.x, r.y, r.w - 12, r.h) } else { r };
    p.text_in(FontId::Regular, 12, lr, Align::Center, label, BLACK);
    if default { glyph_return(p, r.right() - 16, r.y + 9); }
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
    pub mini: Option<usize>, // miniwindow slot
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
    pub fn mini_rect(&self) -> Option<Rect> { self.mini_btn.then(|| rect(self.r.x + 4, self.r.y + 4, 14, 14)) }
    pub fn close_rect(&self) -> Option<Rect> { self.close_btn.then(|| rect(self.r.right() - 18, self.r.y + 4, 14, 14)) }
    pub fn shown(&self) -> bool { self.visible && self.mini.is_none() }

    pub fn hit(&self, p: Pt) -> Option<WinPart> {
        if !self.r.contains(p) { return None; }
        if self.mini_rect().is_some_and(|r| r.contains(p)) { return Some(WinPart::MiniBtn); }
        if self.close_rect().is_some_and(|r| r.contains(p)) { return Some(WinPart::CloseBtn); }
        if self.title_bar().contains(p) { return Some(WinPart::Title); }
        if let Some(rb) = self.resize_bar() {
            if rb.contains(p) {
                let rel = (p.x - rb.x) * 4 / rb.w.max(1);
                return Some(WinPart::Resize(if rel < 1 { -1 } else if rel >= 3 { 1 } else { 0 }));
            }
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
        let tb = self.title_bar();
        p.fill(tb, if key { BLACK } else { DARK });
        if key { p.bevel(tb, DARK, DARK) } else { p.bevel(tb, WHITE, DARK) }
        let title = p.ellipsize(FontId::Bold, 12, &self.title, tb.w - 44);
        p.text_in(FontId::Bold, 12, rect(tb.x + 22, tb.y, tb.w - 44, tb.h), Align::Center, &title, WHITE);
        if let Some(b) = self.mini_rect() { if pressed == Some(WinPart::MiniBtn) { p.pressed(b) } else { p.raised(b) } glyph_mini(p, b); }
        if let Some(b) = self.close_rect() { if pressed == Some(WinPart::CloseBtn) { p.pressed(b) } else { p.raised(b) } glyph_close(p, b); }
        let c = self.content();
        p.fill(c, LIGHT);
        p.bevel(c, WHITE, DARK);
        if let Some(rb) = self.resize_bar() {
            p.fill(rb, LIGHT);
            p.bevel(rb, WHITE, DARK);
            p.vline(rb.x + rb.w / 4, rb.y + 1, rb.h - 2, DARK);
            p.vline(rb.x + rb.w * 3 / 4, rb.y + 1, rb.h - 2, DARK);
        }
    }
}

/// Miniwindow tile (§7.2) for slot `slot` on the Screen's bottom row.
pub fn miniwindow_rect(slot: usize, sh: i32) -> Rect { rect(64 * (slot as i32 + 1), sh - 64, 64, 64) }
pub fn draw_miniwindow(p: &mut Painter, r: Rect, icon: &str, title: &str) {
    p.raised(r);
    p.icon(icon, r.x + 12, r.y + 4, 40);
    let t = p.ellipsize_mid(FontId::Regular, 10, title, 60);
    p.text_in(FontId::Regular, 10, rect(r.x + 2, r.y + 49, 60, 12), Align::Center, &t, BLACK);
}

// ---- scroller (§7.10) -----------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScrollHit { ArrowA, ArrowB, PageBack, PageFwd, Knob }

#[derive(Clone, Copy, Debug)]
pub struct Scroller { pub r: Rect, pub vertical: bool, pub total: i32, pub visible: i32, pub pos: i32 }

impl Scroller {
    pub fn max_pos(&self) -> i32 { (self.total - self.visible).max(0) }
    pub fn fits(&self) -> bool { self.total <= self.visible }
    pub fn len(&self) -> i32 { if self.vertical { self.r.h } else { self.r.w } }
    pub fn arrow_a(&self) -> Rect { if self.vertical { rect(self.r.x, self.r.bottom() - 32, 16, 16) } else { rect(self.r.right() - 32, self.r.y, 16, 16) } }
    pub fn arrow_b(&self) -> Rect { if self.vertical { rect(self.r.x, self.r.bottom() - 16, 16, 16) } else { rect(self.r.right() - 16, self.r.y, 16, 16) } }
    fn knob_len(&self) -> i32 {
        let tl = self.len() - 32;
        ((tl as i64 * self.visible as i64) / self.total.max(1) as i64).max(16).min(tl as i64) as i32
    }
    pub fn knob(&self) -> Option<Rect> {
        if self.fits() { return None; }
        let tl = self.len() - 32;
        let kl = self.knob_len();
        let off = if self.max_pos() == 0 { 0 } else { ((tl - kl) as i64 * self.pos.clamp(0, self.max_pos()) as i64 / self.max_pos() as i64) as i32 };
        Some(if self.vertical { rect(self.r.x, self.r.y + off, 16, kl) } else { rect(self.r.x + off, self.r.y, kl, 16) })
    }
    /// New position for a knob drag of `delta` logical px from `start_pos`.
    pub fn drag_pos(&self, start_pos: i32, delta: i32) -> i32 {
        let free = self.len() - 32 - self.knob_len();
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
        p.sunken(self.r);
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
            p.sunken(rect(c.x - 2, c.y - 2, 4, 4));
        }
    }
}

// ---- menus (§7.3) ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Act {
    None, Disabled, InfoPanel, Open, NewFolder, Duplicate, Destroy, EmptyRecycler, Copy, Paste, SelectAll, CheckDisks,
    ViewBrowser, Scale1, Scale2, ShowHidden, Inspector, ConsoleWin, FileViewerWin, ArrangeFront, Miniaturize, CloseWin, Hide, Quit,
}

pub struct ItemDef { pub label: &'static str, pub key: Option<char>, pub sub: Option<&'static [ItemDef]>, pub act: Act, pub gap: bool }
const fn item(label: &'static str, key: Option<char>, act: Act) -> ItemDef { ItemDef { label, key, sub: None, act, gap: false } }
const fn sub(label: &'static str, sub: &'static [ItemDef]) -> ItemDef { ItemDef { label, key: None, sub: Some(sub), act: Act::None, gap: false } }

pub static SCALE_MENU: [ItemDef; 2] = [item("1×", None, Act::Scale1), item("2×", None, Act::Scale2)];
pub static INFO_MENU: [ItemDef; 3] = [item("Info Panel…", None, Act::InfoPanel), item("Preferences…", None, Act::Disabled), item("Help…", None, Act::Disabled)];
pub static FILE_MENU: [ItemDef; 7] = [
    item("Open", Some('o'), Act::Open), item("Open as Folder", Some('O'), Act::Disabled), item("New Folder", Some('n'), Act::NewFolder),
    item("Duplicate", Some('d'), Act::Duplicate), item("Compress", None, Act::Disabled), item("Destroy", Some('r'), Act::Destroy), item("Empty Recycler", None, Act::EmptyRecycler),
];
pub static EDIT_MENU: [ItemDef; 4] = [item("Cut", Some('x'), Act::Disabled), item("Copy", Some('c'), Act::Copy), item("Paste", Some('v'), Act::Paste), item("Select All", Some('a'), Act::SelectAll)];
pub static DISK_MENU: [ItemDef; 2] = [item("Check for Disks", None, Act::CheckDisks), item("Eject", None, Act::Disabled)];
pub static VIEW_MENU: [ItemDef; 5] = [item("Browser", None, Act::ViewBrowser), item("Icon", None, Act::Disabled), item("Listing", None, Act::Disabled), sub("Scale", &SCALE_MENU), item("Show Hidden Files", None, Act::ShowHidden)];
pub static TOOLS_MENU: [ItemDef; 4] = [item("Inspector…", Some('i'), Act::Inspector), item("Finder…", None, Act::Disabled), item("Processes…", None, Act::Disabled), item("Console…", None, Act::ConsoleWin)];
pub static WINDOWS_MENU: [ItemDef; 4] = [item("File Viewer", None, Act::FileViewerWin), item("Arrange in Front", None, Act::ArrangeFront), item("Miniaturize Window", Some('m'), Act::Miniaturize), item("Close Window", Some('w'), Act::CloseWin)];
pub static SERVICES_MENU: [ItemDef; 1] = [item("No Services Available", None, Act::Disabled)];
pub static MAIN_MENU: [ItemDef; 10] = [
    sub("Info", &INFO_MENU), sub("File", &FILE_MENU), sub("Edit", &EDIT_MENU), sub("Disk", &DISK_MENU), sub("View", &VIEW_MENU),
    sub("Tools", &TOOLS_MENU), sub("Windows", &WINDOWS_MENU), sub("Services", &SERVICES_MENU),
    ItemDef { label: "Hide", key: Some('h'), sub: None, act: Act::Hide, gap: true }, item("Quit", Some('q'), Act::Quit),
];

/// Resolve a menu path (["View", "Scale"]) to its item list.
pub fn resolve_path(path: &[String]) -> Option<&'static [ItemDef]> {
    let mut items: &'static [ItemDef] = &MAIN_MENU;
    for label in path { items = items.iter().find(|i| i.label == label)?.sub?; }
    Some(items)
}
/// Find the item bound to key equivalent `c` anywhere in the tree.
pub fn find_key(items: &'static [ItemDef], c: char) -> Option<&'static ItemDef> {
    items.iter().find_map(|i| if let Some(s) = i.sub { find_key(s, c) } else if i.key == Some(c) { Some(i) } else { None })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuKind { Main, Popup, Sub { parent: usize, item: usize }, Torn }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuPart { Title, Close, Item(usize) }

pub struct MenuInst {
    pub title: String,
    pub items: &'static [ItemDef],
    pub path: Vec<String>,
    pub pos: Pt,
    pub w: i32,
    pub kind: MenuKind,
    pub open_item: Option<usize>, // item whose attached submenu is open (drawn highlighted)
}

impl MenuInst {
    pub fn width(fonts: &Fonts, items: &[ItemDef], title: &str, main: bool) -> i32 {
        if main { return 130; }
        let mut w = fonts.width_px(FontId::Bold, 12, title) as i32 + 8;
        for it in items {
            let lw = fonts.width_px(FontId::Regular, 12, it.label) as i32;
            w = w.max(lw + 8 + 38); // label + left padding + right area (key equivalent or ▸)
        }
        w.max(110)
    }
    pub fn height(&self) -> i32 { MENU_ITEM_H + self.items.iter().map(|i| MENU_ITEM_H + if i.gap { 2 } else { 0 }).sum::<i32>() }
    pub fn rect(&self) -> Rect { rect(self.pos.x, self.pos.y, self.w, self.height()) }
    pub fn title_rect(&self) -> Rect { rect(self.pos.x, self.pos.y, self.w, MENU_ITEM_H) }
    pub fn close_rect(&self) -> Option<Rect> { (self.kind == MenuKind::Torn).then(|| rect(self.pos.x + self.w - 17, self.pos.y + 3, 14, 14)) }
    pub fn item_rect(&self, idx: usize) -> Rect {
        let mut y = self.pos.y + MENU_ITEM_H;
        for (i, it) in self.items.iter().enumerate() {
            if it.gap { y += 2; }
            if i == idx { return rect(self.pos.x, y, self.w, MENU_ITEM_H); }
            y += MENU_ITEM_H;
        }
        rect(self.pos.x, y, self.w, MENU_ITEM_H)
    }
    pub fn hit(&self, p: Pt) -> Option<MenuPart> {
        if !self.rect().contains(p) { return None; }
        if self.close_rect().is_some_and(|r| r.contains(p)) { return Some(MenuPart::Close); }
        if self.title_rect().contains(p) { return Some(MenuPart::Title); }
        (0..self.items.len()).find(|&i| self.item_rect(i).contains(p)).map(MenuPart::Item)
    }

    /// `state(item) -> (disabled, checked)`; `hi` = pressed item; `close_pressed` for torn menus.
    pub fn draw(&self, p: &mut Painter, hi: Option<usize>, close_pressed: bool, state: &dyn Fn(&ItemDef) -> (bool, bool)) {
        let r = self.rect();
        p.outline(rect(r.x - 1, r.y - 1, r.w + 2, r.h + 2), BLACK);
        let tr = self.title_rect();
        p.fill(tr, BLACK);
        p.text_in(FontId::Bold, 12, rect(tr.x + 4, tr.y, tr.w - 8 - if self.kind == MenuKind::Torn { 14 } else { 0 }, tr.h), Align::Center, &self.title, WHITE);
        if let Some(c) = self.close_rect() { if close_pressed { p.pressed(c) } else { p.raised(c) } glyph_close(p, c); }
        for (i, it) in self.items.iter().enumerate() {
            let ir = self.item_rect(i);
            if it.gap { p.fill(rect(ir.x, ir.y - 2, ir.w, 2), DARK); }
            let (disabled, checked) = state(it);
            let inverted = hi == Some(i) || self.open_item == Some(i);
            if inverted { p.fill(ir, BLACK); p.bevel(ir, WHITE, DARK); p.shadow(ir); } else { p.raised(ir); }
            let fg = if inverted { WHITE } else if disabled { DARK } else { BLACK };
            if checked { p.fill(rect(ir.x + 1, ir.y + 7, 6, 6), fg); }
            p.text_in(FontId::Regular, 12, rect(ir.x + 8, ir.y, ir.w - 38, ir.h), Align::Left, it.label, fg);
            if it.sub.is_some() { tri_right(p, ir.right() - 9, ir.y + 7, 6, fg); }
            else if let Some(k) = it.key {
                let kw = p.text_width(FontId::Regular, 12, &k.to_string());
                p.text_in(FontId::Regular, 12, rect(ir.right() - 6 - kw, ir.y, kw, ir.h), Align::Left, &k.to_string(), fg);
                cmd_glyph(p, ir.right() - 6 - kw - 11, ir.y + 6, fg);
            }
        }
    }
}
