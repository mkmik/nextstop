//! Window chrome, scrollers, menus, glyphs — geometry taken from NeXTSTEP 1.0 screenshots (see docs/DECISIONS.md).
use crate::geom::{rect, Pt, Rect};
use crate::paint::*;

pub const TITLE_H: i32 = 22;   // highlight row + 19 fill rows + dark row + black row
pub const RESIZE_H: i32 = 8;   // dark row + white row + 6 face rows
pub const MENU_ITEM_H: i32 = 20;
pub const MENU_TITLE_H: i32 = 22;
pub const SCROLL_W: i32 = 21;  // dark+black frame, 1 track px, 15 px knob/buttons, shadow, light edge, black edge
pub const BTN_H: i32 = 24;

// ---- glyphs -------------------------------------------------------------------------------

/// Filled 7 px triangles for the stepper buttons (Inspector pager, Mandelbrot depth).
pub fn tri_up(p: &mut Painter, x: i32, y: i32, w: i32, color: u32) {
    let half = w / 2;
    for i in 0..=half { p.fill(rect(x + half - i, y + i, 2 * i + 1, 1), color); }
}
pub fn tri_down(p: &mut Painter, x: i32, y: i32, w: i32, color: u32) {
    let half = w / 2;
    for i in 0..=half { p.fill(rect(x + i, y + i, w - 2 * i, 1), color); }
}
/// 2.0 scroller arrow, pixel for pixel (9×9): the triangle grows two pixels every second row and the
/// step corners are dark gray. `fwd` = down / right.
pub fn arrow(p: &mut Painter, x: i32, y: i32, vertical: bool, fwd: bool) {
    const UP: [&str; 9] = ["....D....", "....K....", "...DKD...", "...KKK...", "..DKKKD..", "..KKKKK..", ".DKKKKKD.", ".KKKKKKK.", "DKKKKKKKD"];
    for (i, row) in UP.iter().enumerate() {
        for (j, ch) in row.chars().enumerate() {
            let c = match ch { 'K' => BLACK, 'D' => DARK, _ => continue };
            let (i, j) = (i as i32, j as i32);
            let (dx, dy) = match (vertical, fwd) { (true, false) => (j, i), (true, true) => (j, 8 - i), (false, false) => (i, j), (false, true) => (8 - i, j) };
            p.fill(rect(x + dx, y + dy, 1, 1), c);
        }
    }
}
/// The 2.0 branch / submenu marker, pixel for pixel (7×7): a black bar, a dark upper edge and a white
/// lower edge around an open face — an engraved ▷ rather than an outline. On a white (selected) cell
/// 2.0 draws the lower edge light gray so it stays visible.
pub fn tri_hollow_right(p: &mut Painter, x: i32, y: i32, color: u32, on_white: bool) {
    let lo = if on_white { LIGHT } else { WHITE };
    p.fill(rect(x, y, 1, 7), color);
    p.fill(rect(x + 1, y, 1, 1), DARK); p.fill(rect(x + 1, y + 6, 1, 1), lo);
    for i in 1..3 { p.fill(rect(x + 2 * i, y + i, 2, 1), DARK); p.fill(rect(x + 2 * i, y + 6 - i, 2, 1), lo); }
    p.fill(rect(x + 6, y + 3, 1, 1), lo);
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
/// The round 6×6 knob dimple, pixel for pixel as on the 1.0 scroller (top-left dark, bottom-right white).
pub fn dimple(p: &mut Painter, x: i32, y: i32) {
    const ROWS: [&str; 6] = [".DKKK.", "DKDDDD", "KDD...", "KD..WW", "KD.WWW", ".D.WW."];
    for (dy, row) in ROWS.iter().enumerate() {
        for (dx, ch) in row.chars().enumerate() {
            let c = match ch { 'D' => DARK, 'K' => BLACK, 'W' => WHITE, _ => continue };
            p.fill(rect(x + dx as i32, y + dy as i32, 1, 1), c);
        }
    }
}

/// Raised push button with centred label; `default` adds the return glyph.
pub fn button(p: &mut Painter, r: Rect, label: &str, pressed: bool, default: bool) {
    if pressed { p.pressed(r) } else { p.raised(r) }
    let lr = if default { rect(r.x, r.y, r.w - 14, r.h) } else { r };
    p.text_in(FontId::Regular, 12, lr, Align::Center, label, BLACK);
    if default { glyph_return(p, r.right() - 18, r.y + r.h / 2 - 4); }
}

/// Dock-style tile bevel as in 2.0: 2 px white top/left, 1 px dark + 1 px black bottom/right.
pub fn tile_bevel(p: &mut Painter, r: Rect) {
    p.fill(r, LIGHT);
    p.fill(rect(r.x, r.y, r.w, 2), WHITE); p.fill(rect(r.x, r.y, 2, r.h), WHITE);
    p.fill(rect(r.x + 2, r.bottom() - 2, r.w - 2, 1), DARK); p.fill(rect(r.right() - 2, r.y + 2, 1, r.h - 2), DARK);
    p.fill(rect(r.x + 1, r.bottom() - 1, r.w - 1, 1), BLACK); p.fill(rect(r.right() - 1, r.y + 1, 1, r.h - 1), BLACK);
}

// ---- windows -------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum WinKind { FileViewer, Inspector, Console, Info, Recycler, Mandelbrot, Improv, Shell, Concurrence, Alert }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WinPart { Title, MiniBtn, CloseBtn, Resize(i8), Content }

pub struct Win {
    pub kind: WinKind,
    pub r: Rect,
    pub title: String,
    pub min_w: i32, pub min_h: i32,
    pub resizable: bool, pub mini_btn: bool, pub close_btn: bool, pub chrome: bool,
    pub visible: bool,
    pub mini: Option<usize>,
    pub icon: &'static str,
    pub z: u32,
}

impl Win {
    pub fn new(kind: WinKind, r: Rect, title: &str, icon: &'static str) -> Win {
        Win { kind, r, title: title.into(), min_w: 120, min_h: 60, resizable: true, mini_btn: true, close_btn: true, chrome: true, visible: false, mini: None, icon, z: 0 }
    }
    /// A window without chrome (the Concurrence show) is all content.
    pub fn title_bar(&self) -> Rect { rect(self.r.x, self.r.y, self.r.w, if self.chrome { TITLE_H } else { 0 }) }
    pub fn content(&self) -> Rect { let t = self.title_bar().h; rect(self.r.x, self.r.y + t, self.r.w, self.r.h - t - if self.resizable { RESIZE_H } else { 0 }) }
    pub fn resize_bar(&self) -> Option<Rect> { self.resizable.then(|| rect(self.r.x, self.r.bottom() - RESIZE_H, self.r.w, RESIZE_H)) }
    pub fn mini_rect(&self) -> Option<Rect> { (self.mini_btn && self.chrome).then(|| rect(self.r.x + 3, self.r.y + 3, 14, 14)) }
    pub fn close_rect(&self) -> Option<Rect> { (self.close_btn && self.chrome).then(|| rect(self.r.right() - 17, self.r.y + 3, 14, 14)) }
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
        if !self.chrome { return; }
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
pub struct Scroller { pub r: Rect, pub vertical: bool, pub frame: bool, pub arrows: bool, pub total: i32, pub visible: i32, pub pos: i32 }

/// Framed geometry (vertical): col 0 dark, col 1 black, col 2 one track pixel, cols 3–17 knob/button faces,
/// col 18 their black shadow, col 19 light edge, col 20 black edge; rows mirror this at the top. Unframed strips
/// (browser columns, the icon-path scroller) are 17 px: one track pixel, 15 px faces, one shadow pixel.
/// The two 15 px arrow buttons sit at the end with a one-pixel gap between them, as in 2.0.
impl Scroller {
    pub fn framed(r: Rect, vertical: bool, total: i32, visible: i32, pos: i32) -> Scroller { Scroller { r, vertical, frame: true, arrows: true, total, visible, pos } }
    pub fn max_pos(&self) -> i32 { (self.total - self.visible).max(0) }
    pub fn fits(&self) -> bool { self.total <= self.visible }
    fn len(&self) -> i32 { if self.vertical { self.r.h } else { self.r.w } }
    /// Faces start after the frame (dark, black) and a 1 px light margin; unframed scrollers have the margin only.
    fn inset(&self) -> i32 { if self.frame { 3 } else { 1 } }
    /// 2.0 hides knob and arrows when everything is visible; only the dithered slot remains.
    fn show_arrows(&self) -> bool { self.arrows && !self.fits() }
    /// After the track: margin, face, shadow, gap, face, shadow, margin (35 px) — or just the margin.
    fn tail(&self) -> i32 { if self.show_arrows() { 35 } else { 1 } }
    fn track_len(&self) -> i32 { self.len() - self.inset() - self.tail() }
    pub fn arrow_a(&self) -> Rect { let o = self.inset(); if self.vertical { rect(self.r.x + o, self.r.bottom() - 34, 15, 15) } else { rect(self.r.right() - 34, self.r.y + o, 15, 15) } }
    pub fn arrow_b(&self) -> Rect { let o = self.inset(); if self.vertical { rect(self.r.x + o, self.r.bottom() - 17, 15, 15) } else { rect(self.r.right() - 17, self.r.y + o, 15, 15) } }
    fn knob_len(&self) -> i32 {
        let tl = self.track_len();
        ((tl as i64 * self.visible as i64) / self.total.max(1) as i64).max(16).min(tl as i64) as i32
    }
    /// Knob face (its shadow is drawn outside, like every raised control).
    pub fn knob(&self) -> Option<Rect> {
        if self.fits() { return None; }
        let tl = self.track_len();
        let kl = self.knob_len();
        let off = if self.max_pos() == 0 { 0 } else { ((tl - kl) as i64 * self.pos.clamp(0, self.max_pos()) as i64 / self.max_pos() as i64) as i32 };
        let o = self.inset();
        Some(if self.vertical { rect(self.r.x + o, self.r.y + o + off, 15, kl - 1) } else { rect(self.r.x + o + off, self.r.y + o, kl - 1, 15) })
    }
    pub fn drag_pos(&self, start_pos: i32, delta: i32) -> i32 {
        let free = self.track_len() - self.knob_len();
        if free <= 0 { return start_pos; }
        (start_pos + (delta as i64 * self.max_pos() as i64 / free as i64) as i32).clamp(0, self.max_pos())
    }
    pub fn hit(&self, p: Pt) -> Option<ScrollHit> {
        if !self.r.contains(p) { return None; }
        if self.show_arrows() {
            if self.arrow_a().contains(p) { return Some(ScrollHit::ArrowA); }
            if self.arrow_b().contains(p) { return Some(ScrollHit::ArrowB); }
        }
        let k = self.knob()?;
        if k.contains(p) { return Some(ScrollHit::Knob); }
        let before = if self.vertical { p.y < k.y } else { p.x < k.x };
        Some(if before { ScrollHit::PageBack } else { ScrollHit::PageFwd })
    }
    /// 2.0 geometry, measured: [dark][black] frame, 1 px light margin, a 16 px dithered slot (15 px faces plus
    /// their black shadow), 1 px margin, black separator; the slot stops before the buttons, whose gap and
    /// margin rows stay light.
    pub fn draw(&self, p: &mut Painter, pressed: Option<ScrollHit>) {
        let r = self.r;
        let (o, tl) = (self.inset(), self.track_len());
        p.fill(r, LIGHT);
        p.dither(if self.vertical { rect(r.x + o, r.y + o, 16, tl) } else { rect(r.x + o, r.y + o, tl, 16) });
        if self.frame {
            p.hline(r.x, r.y, r.w, DARK); p.hline(r.x + 1, r.y + 1, r.w - 1, BLACK);
            p.vline(r.x, r.y, r.h, DARK); p.vline(r.x + 1, r.y + 1, r.h - 1, BLACK);
            if self.vertical { p.vline(r.x + 20, r.y, r.h, BLACK) } else { p.hline(r.x, r.y + 20, r.w, BLACK) }
        }
        if self.show_arrows() {
            for (b, hit, fwd) in [(self.arrow_a(), ScrollHit::ArrowA, false), (self.arrow_b(), ScrollHit::ArrowB, true)] {
                if pressed == Some(hit) { p.pressed(b) } else { p.raised(b) }
                arrow(p, b.x + 3, b.y + 3, self.vertical, fwd);
            }
        }
        if let Some(k) = self.knob() {
            p.raised(k);
            dimple(p, k.x + (k.w - 6) / 2, k.y + (k.h - 6) / 2);
        }
    }
}

// ---- menus (§7.3) ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Act {
    None, Disabled, InfoPanel, Open, NewFolder, Duplicate, Destroy, EmptyRecycler, Copy, Paste, SelectAll, CheckDisks,
    ViewBrowser, Scale1, Scale15, Scale2, ShowHidden, Backdrop, ShowDock, ShowMiniwindows, ShowRecycler, Inspector, ConsoleWin, Mandelbrot, Improv, ShellWin, Concurrence, FileViewerWin, RecyclerWin, ArrangeFront, Miniaturize, CloseWin, Hide, Quit,
}

pub struct ItemDef { pub label: &'static str, pub key: Option<char>, pub sub: Option<&'static [ItemDef]>, pub act: Act }
const fn item(label: &'static str, key: Option<char>, act: Act) -> ItemDef { ItemDef { label, key, sub: None, act } }
const fn sub(label: &'static str, sub: &'static [ItemDef]) -> ItemDef { ItemDef { label, key: None, sub: Some(sub), act: Act::None } }

pub static SCALE_MENU: [ItemDef; 3] = [item("1×", None, Act::Scale1), item("1.5×", None, Act::Scale15), item("2×", None, Act::Scale2)];
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
pub static TOOLS_MENU: [ItemDef; 8] = [item("Inspector…", Some('i'), Act::Inspector), item("Finder…", None, Act::Disabled), item("Processes…", None, Act::Disabled), item("Console…", None, Act::ConsoleWin), item("Shell…", Some('t'), Act::ShellWin), item("Concurrence…", None, Act::Concurrence), item("Mandelbrot…", None, Act::Mandelbrot), item("Improv…", None, Act::Improv)];
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
pub enum MenuKind { Main, Popup, Sub { parent: u64, item: usize }, Torn }

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
    /// Face rectangle (the black shadow column lies just outside it, on the right).
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
        // the only drop shadow is one black column on the right; the bottom ends with the last cell's black row (2.0 shots)
        p.vline(r.right(), r.y, r.h, BLACK);
        // title: raised black cell (white highlight, dark shadow) + black shadow row
        let tr = self.title_rect();
        p.fill(rect(tr.x, tr.y, tr.w, 21), BLACK);
        p.bevel(rect(tr.x, tr.y, tr.w, 21), WHITE, DARK);
        p.hline(tr.x, tr.y + 21, tr.w, BLACK);
        let tw = tr.w - 6 - if self.kind == MenuKind::Torn { 16 } else { 0 };
        p.text_in(FontId::Bold, 12, rect(tr.x + 5, tr.y, tw, 20), Align::Left, &p.ellipsize(FontId::Bold, 12, &self.title, tw), WHITE);
        if let Some(c) = self.close_rect() { if close_pressed { p.pressed(c) } else { p.raised(c) } glyph_close(p, c); }
        for (i, it) in self.items.iter().enumerate() {
            let ir = self.item_rect(i);
            let (disabled, checked) = state(it);
            let inverted = hi == Some(i) || self.open_item == Some(i);
            let face = rect(ir.x, ir.y, ir.w, 19);
            p.fill(face, if inverted { WHITE } else { LIGHT }); // 2.0: pressed / open items turn white
            p.bevel(face, WHITE, DARK);
            p.hline(ir.x, ir.bottom() - 1, ir.w, BLACK);
            let fg = if disabled { DARK } else { BLACK };
            if checked { p.fill(rect(ir.x + 1, ir.y + 7, 4, 4), fg); }
            p.text_in(FontId::Regular, 12, rect(ir.x + 6, ir.y, ir.w - 34, 19), Align::Left, it.label, fg);
            if it.sub.is_some() { tri_hollow_right(p, ir.right() - 12, ir.y + 6, fg, inverted); }
            else if let Some(k) = it.key {
                let kw = p.text_width(FontId::Regular, 12, &k.to_string());
                p.text_in(FontId::Regular, 12, rect(ir.right() - 6 - kw, ir.y, kw, 19), Align::Left, &k.to_string(), fg);
            }
        }
    }
}
