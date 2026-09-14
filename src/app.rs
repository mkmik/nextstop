//! The application: fake Screen, windows, menus, Dock… All coordinates are logical NeXT pixels.
use crate::backend::{apps, fs, state};
use crate::backend::fs::Entry;
use crate::chrome::*;
use crate::dock::{Dock, RecWin, TileId};
use crate::fileviewer::FileViewer;
use crate::geom::{pt, rect, Pt, Rect};
use crate::icons;
use crate::inspector::Inspector;
use crate::mandel::{MBtn, Mandel};
use crate::shell::Shell;
use crate::paint::*;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods { pub shift: bool, pub alt: bool, pub ctrl: bool, pub cmd: bool }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button { Left, Right, Other }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key { Up, Down, Left, Right, Enter, Escape, Delete, Backspace, Tab, Home, End, PageUp, PageDown, Char(char) }

#[derive(Clone, Copy, Debug)]
pub enum Ev { MouseMove(Pt), MouseDown(Pt, Button, Mods), MouseUp(Pt, Button, Mods), Wheel(Pt, f32, f32), Key(Key, Mods), Focus(bool) }

/// Background job results, delivered on the UI thread.
#[derive(Debug)]
pub enum Job {
    Listed { col_id: u64, seq: u64, result: Result<Vec<Entry>, String>, quiet: bool },
    Meta { path: String, result: Result<fs::Meta, String> },
    Text { path: String, result: Result<String, String> },
    ImageBytes { path: String, result: Result<Vec<u8>, String> },
    DirSize { path: String, result: Result<u64, String> },
    AppIcon { app: String, result: Result<Vec<u8>, String> },
    TrashState(Result<bool, String>),
    TrashList { seq: u64, result: Result<Vec<Entry>, String> },
    Done { what: String, result: Result<(), String> },
    NewFolder(Result<String, String>),
    Mandel { seq: u64, iters: Vec<u16>, ms: u32 },
    TermWake, TermTitle(String), TermWrite(String), TermExit,
}

pub struct Config { pub demo: Option<String>, pub home_override: Option<String>, pub config_override: Option<String>, pub scale_override: Option<f32> }

/// What a modal alert does when its confirming (rightmost) button is pressed.
#[derive(Clone, Debug)]
pub enum Pending { Nothing, Trash(Vec<String>), Destroy(Vec<String>), EmptyTrash }

pub struct Alert { pub message: String, pub detail: String, pub buttons: Vec<String>, pub pending: Pending, pub prev_key: Option<WinKind> }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScrollId { Col(u64), Browser, Console, InspText, Recycler, Shell }

/// Every press-and-release control on the Screen (§9.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum Btn {
    WinMini(WinKind), WinClose(WinKind), MenuClose(u64), MenuItem(u64, usize), AlertBtn(usize),
    Tile(TileId), Miniwin(WinKind), ScrollArrow(ScrollId, i8), Shelf(usize), PathItem(usize),
    InspPopup, InspRow(usize), InspCompute, RecBtn, Cell(usize, usize), Mandel(MBtn),
}

pub enum Capture {
    Press(Btn),
    WinMove { k: WinKind, grab: Pt },
    WinResize { k: WinKind, start: Pt, orig: Rect, region: i8 },
    MenuDrag { m: u64, grab: Pt, moved: bool },
    Knob { id: ScrollId, start: Pt, start_pos: i32 },
    CellPress { col: usize, idx: usize, start: Pt },
    ShelfPress { idx: usize, start: Pt },
    FileDrag { paths: Vec<String>, icon: &'static str, from_shelf: Option<usize> },
    TilePress { idx: usize, start: Pt, dx: i32, dy: i32 },
    MandelDrag { start: Pt },
}

/// One real OS window in multi-window mode (or one layer of the headless composite).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SurfaceId { Backdrop, Win(WinKind), Menu(u64), Dock, Recycler, AppTile, Miniwin(WinKind), Ghost }
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level { Bottom, Normal, Top }
pub struct SurfaceInfo { pub id: SurfaceId, pub r: Rect, pub level: Level, pub title: String }

pub struct App {
    pub w: i32, pub h: i32,
    pub zoom: f32,
    pub quit: bool,
    redraw: bool,
    minimize: bool,
    pub cfg: Config,
    pub post: Arc<dyn Fn(Job) + Send + Sync>,
    pub fonts: Rc<Fonts>,
    pub home: String, pub roots: Vec<String>, pub is_win: bool,
    pub state: state::State,
    save_at: Option<Instant>,
    pub wins: Vec<Win>,
    pub key: Option<WinKind>,
    activations: Vec<WinKind>,
    zc: u32,
    menu_seq: u64,
    pub menus: Vec<MenuInst>,
    pub alert: Option<Alert>,
    pub fv: FileViewer, pub insp: Inspector, pub dock: Dock, pub rec: RecWin, pub mandel: Mandel, pub shell: Shell,
    pub console: Vec<String>, pub console_scroll: i32, console_stick: bool,
    pub capture: Option<Capture>,
    pub mouse: Pt,
    focused: bool,
    refresh_at: Instant,
    pub clipboard: Vec<String>,
    pub now: Instant,
}

const KINDS: [WinKind; 8] = [WinKind::FileViewer, WinKind::Inspector, WinKind::Console, WinKind::Info, WinKind::Recycler, WinKind::Mandelbrot, WinKind::Shell, WinKind::Alert];
pub fn wi(k: WinKind) -> usize { KINDS.iter().position(|x| *x == k).unwrap() }

impl App {
    pub fn new(cfg: Config, post: Arc<dyn Fn(Job) + Send + Sync>, fonts: Rc<Fonts>) -> App {
        if let Some(h) = &cfg.home_override { fs::set_home_override(h); }
        if let Some(c) = &cfg.config_override { apps::set_config_override(c); }
        let (loaded, msg) = state::read_state(&state::state_path());
        Self::from_state(cfg, post, fonts, loaded, msg)
    }

    fn from_state(cfg: Config, post: Arc<dyn Fn(Job) + Send + Sync>, fonts: Rc<Fonts>, loaded: Option<state::State>, msg: Option<String>) -> App {
        let home = fs::home_dir().unwrap_or_else(|_| "/".into());
        let roots = fs::root_dirs();
        let is_win = roots.first().is_some_and(|r| r != "/");
        let fresh = loaded.is_none();
        let mut st = loaded.unwrap_or_default();
        let mut console = vec![];
        let mut log = |s: String| console.push(format!("{} {s}", ts()));
        if let Some(m) = msg { log(m); }
        if fresh {
            st.dock = apps::default_dock();
            let apps_dir = if is_win { "C:\\Program Files".to_string() } else { "/Applications".into() };
            st.shelf = vec![home.clone(), roots.first().cloned().unwrap_or("/".into()), apps_dir, icons::join(&home, "Desktop"), icons::join(&home, "Documents")];
        }
        if st.windows.file_viewer.path.is_empty() { st.windows.file_viewer.path = home.clone(); }
        // §11: drop paths that no longer exist
        for (list, what) in [(&mut st.dock, "Dock"), (&mut st.shelf, "Shelf")] {
            let keep = fs::filter_existing(list.clone());
            for p in list.iter().filter(|p| !keep.contains(p)) { log(format!("dropped missing {what} entry {p}")); }
            *list = keep;
        }
        if fs::filter_existing(vec![st.windows.file_viewer.path.clone()]).is_empty() { st.windows.file_viewer.path = home.clone(); }
        if !(0.5..=4.0).contains(&st.scale) { st.scale = 1.0; }
        let wr = |w: &state::WinState| rect(w.x, w.y, w.w, w.h);
        let mut wins = vec![
            Win::new(WinKind::FileViewer, wr(&st.windows.file_viewer), "File Viewer", "folder"),
            Win::new(WinKind::Inspector, rect(st.windows.inspector.x, st.windows.inspector.y, 272, 400), "Inspector", "miniwindow"),
            Win::new(WinKind::Console, wr(&st.windows.console), "Console", "miniwindow"),
            Win::new(WinKind::Info, rect(0, 120, 300, 200), "Info", "workspace"),
            Win::new(WinKind::Recycler, wr(&st.windows.recycler), "Recycler", "recycler-empty"),
            Win::new(WinKind::Mandelbrot, rect(st.windows.mandelbrot.x, st.windows.mandelbrot.y, crate::mandel::WIN_W, crate::mandel::WIN_H), "Mandelbrot", "file-image"),
            Win::new(WinKind::Shell, wr(&st.windows.shell), "Shell", "miniwindow"),
            Win::new(WinKind::Alert, rect(0, 0, 380, 176), "", "alert"),
        ];
        wins[wi(WinKind::FileViewer)].min_w = 480; wins[wi(WinKind::FileViewer)].min_h = 320;
        wins[wi(WinKind::Inspector)].resizable = false; wins[wi(WinKind::Inspector)].mini_btn = false;
        wins[wi(WinKind::Console)].min_w = 240; wins[wi(WinKind::Console)].min_h = 100;
        wins[wi(WinKind::Info)].resizable = false; wins[wi(WinKind::Info)].mini_btn = false;
        wins[wi(WinKind::Recycler)].min_w = 200; wins[wi(WinKind::Recycler)].min_h = 120;
        wins[wi(WinKind::Mandelbrot)].resizable = false;
        wins[wi(WinKind::Shell)].min_w = 20 * crate::shell::CELL_W + SCROLL_W + 2 * crate::shell::PAD; wins[wi(WinKind::Shell)].min_h = 5 * crate::shell::CELL_H + 2 * crate::shell::PAD + TITLE_H + RESIZE_H;
        let a = &mut wins[wi(WinKind::Alert)];
        a.resizable = false; a.mini_btn = false; a.close_btn = false;
        let zoom = cfg.scale_override.filter(|z| (0.5..=4.0).contains(z)).unwrap_or(st.scale);
        let mut app = App {
            w: st.os_window.w, h: st.os_window.h, zoom, quit: false, redraw: true, minimize: false, cfg, post, fonts,
            home: home.clone(), roots, is_win, state: st, save_at: None, wins, key: None, activations: vec![], zc: 0, menu_seq: 0, menus: vec![], alert: None,
            fv: FileViewer::default(), insp: Inspector::default(), dock: Dock::default(), rec: RecWin::default(), mandel: Mandel::default(), shell: Shell::default(),
            console, console_scroll: 0, console_stick: true, capture: None, mouse: pt(0, 0), focused: true,
            refresh_at: Instant::now() + Duration::from_secs(5), clipboard: vec![], now: Instant::now(),
        };
        app.build_menus();
        app
    }

    #[cfg(test)]
    pub(crate) fn test_app() -> App {
        Self::from_state(Config { demo: None, home_override: None, config_override: None, scale_override: None },
            Arc::new(|_| {}), Rc::new(Fonts::load()), Some(state::State::default()), None)
    }


    /// Called once the window exists (or the headless harness is set up).
    pub fn start(&mut self) {
        if self.cfg.demo.is_some() { return; }
        if self.state.windows.file_viewer.open { self.show_win(WinKind::FileViewer); }
        if self.state.windows.inspector.open { self.show_win(WinKind::Inspector); }
        if self.state.windows.console.open { self.show_win(WinKind::Console); }
        if self.state.windows.recycler.open { self.show_win(WinKind::Recycler); }
        if self.state.windows.mandelbrot.open { self.show_win(WinKind::Mandelbrot); }
        if self.state.windows.shell.open { self.show_win(WinKind::Shell); }
        if self.state.windows.file_viewer.open { self.activate_win(WinKind::FileViewer); }
        self.log("ReWorkspace started".into());
        let path = self.state.windows.file_viewer.path.clone();
        self.fv_navigate(&path);
        self.update_trash();
        self.dock_request_icons();
    }

    // ---- plumbing --------------------------------------------------------------------------
    pub fn resize(&mut self, w: i32, h: i32) {
        self.w = w; self.h = h;
        for win in &mut self.wins { win.clamp(w, h); }
        self.redraw = true;
    }
    pub fn take_redraw(&mut self) -> bool { std::mem::take(&mut self.redraw) }
    pub fn wants_minimize(&mut self) -> bool { std::mem::take(&mut self.minimize) }
    pub fn cursor_kind(&self) -> usize {
        match &self.capture {
            Some(Capture::WinResize { region, .. }) => return match region { -1 => 1, 0 => 2, _ => 3 },
            Some(_) => return 0,
            None => {}
        }
        if self.alert.is_none() {
            if let Some((k, WinPart::Resize(r))) = self.win_hit(self.mouse) { if self.wins[wi(k)].shown() { return match r { -1 => 1, 0 => 2, _ => 3 }; } }
        }
        0
    }
    pub fn spawn(&self, f: impl FnOnce() -> Job + Send + 'static) {
        let post = self.post.clone();
        std::thread::spawn(move || post(f()));
    }
    pub fn log(&mut self, msg: String) {
        let line = format!("{} {msg}", ts());
        eprintln!("{line}");
        self.console.push(line);
        if self.console.len() > 500 { self.console.remove(0); }
        self.console_stick = true;
        self.redraw = true;
    }
    pub fn console_tail(&self, n: usize) -> Vec<String> { self.console.iter().rev().take(n).rev().cloned().collect() }
    pub fn dirty(&mut self) { self.save_at = Some(self.now + Duration::from_secs(2)); }
    pub fn save_now(&mut self) {
        self.save_at = None;
        if self.cfg.demo.is_some() { return; }
        self.state.os_window = state::WH { w: (self.w as f32 * self.zoom) as i32, h: (self.h as f32 * self.zoom) as i32 };
        if let Err(e) = state::save_state(&self.state) { self.log(format!("save failed: {e}")); }
    }
    pub fn tick(&mut self, now: Instant) {
        self.now = now;
        if self.save_at.is_some_and(|t| now >= t) { self.save_now(); }
        if now >= self.refresh_at {
            self.refresh_at = now + Duration::from_secs(5);
            if self.focused && self.cfg.demo.is_none() { self.fv_refresh(); self.update_trash(); }
        }
        if self.fv.type_until.is_some_and(|t| now >= t) { self.fv.type_until = None; self.fv.typeahead.clear(); }
        if self.dock.pressed_until.is_some_and(|(_, t)| now >= t) { self.dock.pressed_until = None; self.redraw = true; }
    }
    pub fn next_deadline(&self) -> Option<Instant> {
        let mut t: Option<Instant> = None;
        let mut add = |x: Option<Instant>| if let Some(x) = x { t = Some(t.map_or(x, |c| c.min(x))) };
        add(self.save_at);
        add(Some(self.refresh_at));
        add(self.fv.type_until);
        add(self.dock.pressed_until.map(|(_, t)| t));
        t
    }
    pub fn debug_query(&self, q: &str) -> String {
        match q {
            "title" => self.wins[wi(WinKind::FileViewer)].title.clone(),
            "dock" => self.state.dock.join(","),
            "shelf" => self.state.shelf.join(","),
            "menus" => self.menus.iter().map(|m| format!("{}@{},{}:{:?}", m.title, m.pos.x, m.pos.y, m.kind)).collect::<Vec<_>>().join(" "),
            "key" => format!("{:?}", self.key),
            "zoom" => format!("{}", self.zoom),
            "hscroll" => self.fv_layout().hscroll.pos.to_string(),
            "surfaces" => self.surfaces().iter().map(|s| format!("{:?}", s.id)).collect::<Vec<_>>().join(" "),
            "selection" => self.fv_deep_selection().iter().map(|e| e.path.clone()).collect::<Vec<_>>().join(","),
            "cols" => self.fv.cols.iter().map(|c| format!("{}({})", c.dir.clone().unwrap_or("Computer".into()), c.entries.len())).collect::<Vec<_>>().join(" | "),
            "state" => serde_json::to_string(&self.state).unwrap_or_default(),
            "selrect" => self.fv_sel_cell_rect().map_or("none".into(), |r| format!("{} {} {} {}", r.x, r.y, r.w, r.h)),
            q if q.starts_with("cellrect ") => {
                let idx: usize = q[9..].trim().parse().unwrap_or(0);
                self.fv_cell_rect_of(self.fv.focus, idx).map_or("none".into(), |r| format!("{} {} {} {}", r.x, r.y, r.w, r.h))
            }
            _ => "?".into(),
        }
    }

    // ---- windows -----------------------------------------------------------------------------
    pub fn win(&self, k: WinKind) -> &Win { &self.wins[wi(k)] }
    pub fn win_mut(&mut self, k: WinKind) -> &mut Win { &mut self.wins[wi(k)] }
    pub fn front(&mut self, k: WinKind) { self.zc += 1; self.wins[wi(k)].z = self.zc; self.redraw = true; }
    pub fn make_key(&mut self, k: WinKind) {
        self.front(k);
        if self.key != Some(k) { self.key = Some(k); }
    }
    /// Explicit user activation must also raise/focus the existing OS window.
    /// Native focus notifications use make_key alone to avoid a feedback loop.
    fn activate_win(&mut self, k: WinKind) {
        self.make_key(k);
        self.activations.retain(|w| *w != k);
        self.activations.push(k);
    }
    pub fn take_activations(&mut self) -> Vec<WinKind> { std::mem::take(&mut self.activations) }
    fn state_win(&mut self, k: WinKind) -> Option<&mut state::WinState> {
        let w = &mut self.state.windows;
        match k { WinKind::FileViewer => Some(&mut w.file_viewer), WinKind::Inspector => Some(&mut w.inspector), WinKind::Console => Some(&mut w.console), WinKind::Recycler => Some(&mut w.recycler), WinKind::Mandelbrot => Some(&mut w.mandelbrot), WinKind::Shell => Some(&mut w.shell), _ => None }
    }
    fn sync_win_state(&mut self, k: WinKind) {
        let (r, open) = { let w = self.win(k); (w.r, w.visible) };
        if let Some(s) = self.state_win(k) { s.x = r.x; s.y = r.y; s.w = r.w; s.h = r.h; s.open = open; self.dirty(); }
    }
    pub fn show_win(&mut self, k: WinKind) {
        if self.win(k).mini.is_some() { self.restore(k); }
        let (w, h) = (self.w, self.h);
        let win = self.win_mut(k);
        win.visible = true;
        win.clamp(w, h);
        if k == WinKind::Info { win.r.x = (w - win.r.w) / 2; }
        self.activate_win(k);
        self.sync_win_state(k);
        if k == WinKind::Recycler { self.rec_open(); }
        if k == WinKind::Inspector { self.insp_refresh(); }
        if k == WinKind::Mandelbrot { self.mandel_ensure(); }
        if k == WinKind::Shell { self.shell_start(); }
        self.redraw = true;
    }
    pub fn close_win(&mut self, k: WinKind) {
        if k == WinKind::Alert { return; }
        if k == WinKind::Shell { self.shell_stop(); }
        self.win_mut(k).visible = false;
        self.win_mut(k).mini = None;
        self.sync_win_state(k);
        self.drop_key(k);
        self.redraw = true;
    }
    /// Key window went away: give key to the topmost remaining window.
    fn drop_key(&mut self, k: WinKind) {
        if self.key != Some(k) { return; }
        self.key = self.wins.iter().filter(|w| w.shown() && w.kind != k && w.kind != WinKind::Alert).max_by_key(|w| w.z).map(|w| w.kind);
    }
    pub fn miniaturize(&mut self, k: WinKind) {
        if self.win(k).mini.is_some() || !self.win(k).visible || k == WinKind::Alert { return; }
        let slot = self.wins.iter().filter(|w| w.mini.is_some()).count();
        self.win_mut(k).mini = Some(slot);
        self.drop_key(k);
        self.redraw = true;
    }
    pub fn restore(&mut self, k: WinKind) {
        if self.win(k).mini.is_none() { return; }
        self.win_mut(k).mini = None;
        // re-pack remaining miniwindows
        let mut n = 0;
        for w in &mut self.wins { if w.mini.is_some() { w.mini = Some(n); n += 1; } }
        self.activate_win(k);
        self.redraw = true;
    }
    /// The OS raised this window (clicked); keep the model's z-order in step before hit-testing.
    pub fn raise(&mut self, k: WinKind) { if self.win(k).shown() { self.front(k); } }
    /// The OS moved a surface (constrained to the visible area, etc.): follow it in the model.
    pub fn surface_moved(&mut self, id: SurfaceId, x: i32, y: i32) {
        match id {
            SurfaceId::Win(k) => { let w = self.win_mut(k); if w.r.x != x + 1 || w.r.y != y + 1 { w.r.x = x + 1; w.r.y = y + 1; self.sync_win_state(k); } }
            SurfaceId::Menu(mid) => {
                if let Some(i) = self.menus.iter().position(|m| m.id == mid) {
                    if self.menus[i].pos != pt(x, y) { self.menus[i].pos = pt(x, y); self.reattach(i); self.menu_moved(i); }
                }
            }
            _ => {}
        }
    }
    pub fn has_open_menus(&self) -> bool { self.menus.iter().any(|m| !matches!(m.kind, MenuKind::Main | MenuKind::Torn) || m.open_item.is_some()) }
    pub fn close_open_menus(&mut self) { self.close_submenus(); }
    /// Route native Alt-F4/taskbar Close through the same model operations as our chrome.
    pub fn close_surface(&mut self, id: SurfaceId) {
        self.capture = None;
        match id {
            SurfaceId::Win(WinKind::Alert) => self.alert_button(0),
            SurfaceId::Win(k) | SurfaceId::Miniwin(k) => self.close_win(k),
            SurfaceId::Menu(id) => {
                if let Some(i) = self.menu_index(id) {
                    match self.menus[i].kind {
                        MenuKind::Main => self.act(Act::Quit),
                        MenuKind::Torn => self.close_torn(id),
                        _ => self.close_submenus(),
                    }
                }
            }
            SurfaceId::AppTile => self.act(Act::Quit),
            SurfaceId::Backdrop => { self.state.backdrop = false; self.dirty(); }
            SurfaceId::Dock => { self.state.dock_visible = false; self.dirty(); }
            SurfaceId::Recycler => { self.state.recycler_visible = false; self.dirty(); }
            SurfaceId::Ghost => {}
        }
        self.redraw = true;
    }
    /// Topmost shown window under `p` (alert excluded unless open).
    pub fn win_hit(&self, p: Pt) -> Option<(WinKind, WinPart)> {
        let mut best: Option<&Win> = None;
        for w in &self.wins {
            if !w.shown() || w.kind == WinKind::Alert || !w.r.contains(p) { continue; }
            if best.is_none_or(|b| w.z > b.z) { best = Some(w); }
        }
        best.and_then(|w| w.hit(p).map(|part| (w.kind, part)))
    }
    pub fn content_rect(&self, k: WinKind) -> Rect { self.win(k).content() }

    // ---- alert (§7.11) ------------------------------------------------------------------------
    pub fn show_alert(&mut self, message: &str, detail: &str, buttons: &[&str], pending: Pending) {
        let prev_key = self.key;
        let (w, h) = (self.w, self.h);
        let a = self.win_mut(WinKind::Alert);
        a.r = rect((w - 380) / 2, (h - 176) / 2, 380, 176);
        a.visible = true;
        self.alert = Some(Alert { message: message.into(), detail: detail.into(), buttons: buttons.iter().map(|s| s.to_string()).collect(), pending, prev_key });
        self.capture = None;
        self.activate_win(WinKind::Alert);
        self.redraw = true;
    }
    fn alert_button(&mut self, i: usize) {
        let Some(a) = self.alert.take() else { return };
        self.win_mut(WinKind::Alert).visible = false;
        self.key = a.prev_key.filter(|k| self.win(*k).shown());
        if i == a.buttons.len() - 1 && a.buttons.len() > 1 {
            match a.pending {
                Pending::Nothing => {}
                Pending::Trash(paths) => self.trash_paths(paths),
                Pending::Destroy(paths) => self.run_fs(format!("destroy {} item(s)", paths.len()), move || fs::destroy_paths(paths)),
                Pending::EmptyTrash => { self.run_fs("emptied the Recycler".into(), fs::empty_trash); }
            }
        }
        self.redraw = true;
    }
    pub fn error(&mut self, message: &str, detail: String) { self.log(format!("{message}: {detail}")); self.show_alert(message, &detail, &["OK"], Pending::Nothing); }
    fn alert_buttons(&self) -> Vec<Rect> {
        let Some(a) = &self.alert else { return vec![] };
        let c = self.content_rect(WinKind::Alert);
        let n = a.buttons.len() as i32;
        (0..n).map(|i| rect(c.right() - 10 - (n - i) * 88 + 8, c.bottom() - 10 - BTN_H, 80, BTN_H - 1)).collect()
    }

    // ---- fs operations ------------------------------------------------------------------------
    /// Run a mutating filesystem command off-thread; log the outcome and refresh.
    pub fn run_fs(&mut self, what: String, f: impl FnOnce() -> Result<(), String> + Send + 'static) {
        self.spawn(move || Job::Done { what, result: f() });
    }
    pub fn trash_paths(&mut self, paths: Vec<String>) {
        let n = paths.len();
        self.run_fs(format!("moved {n} item(s) to the Recycler"), move || fs::trash_paths(paths));
    }
    pub fn open_paths(&mut self, entries: &[Entry]) {
        for e in entries.iter().filter(|e| !e.is_dir) {
            self.log(format!("open {}", e.path));
            if let Err(err) = apps::open_path(&e.path) { self.error("Cannot open", err); }
        }
    }
    pub fn update_trash(&mut self) { self.spawn(|| Job::TrashState(fs::trash_is_empty())); }

    pub fn on_job(&mut self, j: Job) {
        self.redraw = true;
        match j {
            Job::Listed { col_id, seq, result, quiet } => self.fv_listed(col_id, seq, result, quiet),
            Job::Meta { path, result } => self.insp_meta(path, result),
            Job::Text { path, result } => self.insp_text(path, result),
            Job::ImageBytes { path, result } => self.insp_image(path, result),
            Job::DirSize { path, result } => self.insp_dir_size(path, result),
            Job::AppIcon { app, result } => self.dock_icon(app, result),
            Job::TrashState(r) => self.dock_trash_state(r),
            Job::TrashList { seq, result } => { if seq == self.rec.seq { self.rec.items = Some(result); self.rec.scroll = self.rec_scroller().pos; } }
            Job::Done { what, result } => {
                match result { Ok(()) => self.log(what), Err(e) => self.error("Operation failed", e) }
                self.fv_refresh();
                self.update_trash();
            }
            Job::Mandel { seq, iters, ms } => self.mandel_done(seq, iters, ms),
            Job::TermWake => {}
            Job::TermTitle(t) => self.shell_title(t),
            Job::TermWrite(s) => self.shell_write(s),
            Job::TermExit => self.shell_exited(),
            Job::NewFolder(r) => match r {
                Ok(p) => { self.log(format!("new folder {p}")); self.fv_navigate(&p); }
                Err(e) => self.error("Cannot create folder", e),
            },
        }
    }

    // ---- menus (§7.3) -------------------------------------------------------------------------
    fn build_menus(&mut self) {
        let w = MenuInst::width(&self.fonts, &MAIN_MENU, "Workspace");
        let id = self.next_menu_id();
        self.menus.push(MenuInst { id, title: "Workspace".into(), items: &MAIN_MENU, path: vec![], pos: pt(self.state.menu_pos.x, self.state.menu_pos.y), w, kind: MenuKind::Main, open_item: None });
        for t in self.state.torn_menus.clone() {
            if let Some(items) = resolve_path(&t.path) {
                let title = t.path.last().cloned().unwrap_or_default();
                let w = MenuInst::width(&self.fonts, items, &title);
                let id = self.next_menu_id();
                self.menus.push(MenuInst { id, title, items, path: t.path.clone(), pos: pt(t.x, t.y), w, kind: MenuKind::Torn, open_item: None });
            }
        }
    }
    fn next_menu_id(&mut self) -> u64 { self.menu_seq += 1; self.menu_seq }
    pub fn item_state(&self, it: &ItemDef) -> (bool, bool) {
        match it.act {
            Act::Disabled => (true, false),
            Act::Paste => (self.clipboard.is_empty(), false),
            Act::EmptyRecycler => (!cfg!(target_os = "macos"), false),
            Act::Hide => (!cfg!(target_os = "macos"), false),
            Act::ViewBrowser => (false, true),
            Act::Scale1 => (false, (self.zoom - 1.0).abs() < 0.01),
            Act::Scale15 => (false, (self.zoom - 1.5).abs() < 0.01),
            Act::Scale2 => (false, (self.zoom - 2.0).abs() < 0.01),
            Act::ShowHidden => (false, self.state.show_hidden),
            Act::Backdrop => (false, self.state.backdrop),
            Act::ShowDock => (false, self.state.dock_visible),
            Act::ShowMiniwindows => (false, self.state.miniwindows_visible),
            Act::ShowRecycler => (false, self.state.recycler_visible),
            _ => (false, false),
        }
    }
    /// Close attached submenus everywhere and destroy popups; torn-off menus stay (§9.7).
    fn close_submenus(&mut self) {
        self.menus.retain(|m| matches!(m.kind, MenuKind::Main | MenuKind::Torn));
        for m in &mut self.menus { m.open_item = None; }
        self.redraw = true;
    }
    fn menu_index(&self, id: u64) -> Option<usize> { self.menus.iter().position(|m| m.id == id) }
    fn close_children_of(&mut self, m: u64) {
        // IDs survive both reordering and recursive removal of lower vector positions.
        while let Some(i) = self.menus.iter().position(|x| matches!(x.kind, MenuKind::Sub { parent, .. } if parent == m)) {
            let child = self.menus[i].id;
            self.close_children_of(child);
            self.menus.retain(|x| x.id != child);
        }
        if let Some(i) = self.menu_index(m) { self.menus[i].open_item = None; }
    }
    fn open_submenu(&mut self, parent: u64, item: usize) {
        let Some(m) = self.menu_index(parent) else { return };
        if self.menus[m].open_item == Some(item) { self.close_children_of(parent); return; }
        self.close_children_of(parent);
        let Some(m) = self.menu_index(parent) else { return };
        let it = &self.menus[m].items[item];
        let Some(items) = it.sub else { return };
        let mut path = self.menus[m].path.clone();
        path.push(it.label.to_string());
        if let Some(t) = self.menus.iter().position(|x| x.kind == MenuKind::Torn && x.path == path) {
            let t = self.menus.remove(t); self.menus.push(t); // already torn off: bring it forward
            return;
        }
        let title = it.label.to_string();
        let w = MenuInst::width(&self.fonts, items, &title);
        self.menus[m].open_item = Some(item);
        let id = self.next_menu_id();
        self.menus.push(MenuInst { id, title, items, path, pos: pt(0, 0), w, kind: MenuKind::Sub { parent, item }, open_item: None });
        let i = self.menus.len() - 1;
        self.reattach(i);
    }
    /// Attached submenus sit right of their parent (1 px shadow + 1 px gap), tops aligned, as in 1.0.
    fn reattach(&mut self, i: usize) {
        if let MenuKind::Sub { parent, .. } = self.menus[i].kind {
            let Some(parent) = self.menu_index(parent) else { return };
            let p = &self.menus[parent];
            self.menus[i].pos = pt(p.pos.x + p.w + 2, p.pos.y);
        }
        let id = self.menus[i].id;
        let children: Vec<usize> = self.menus.iter().enumerate().filter(|(_, x)| matches!(x.kind, MenuKind::Sub { parent, .. } if parent == id)).map(|(j, _)| j).collect();
        for c in children { self.reattach(c); }
    }
    fn tear_off(&mut self, i: usize) {
        if let MenuKind::Sub { parent, .. } = self.menus[i].kind {
            if let Some(parent) = self.menu_index(parent) { self.menus[parent].open_item = None; }
            self.menus[i].kind = MenuKind::Torn;
            let path = self.menus[i].path.clone();
            let pos = self.menus[i].pos;
            if !self.state.torn_menus.iter().any(|t| t.path == path) { self.state.torn_menus.push(state::TornMenu { path, x: pos.x, y: pos.y }); }
            self.dirty();
        }
    }
    fn menu_moved(&mut self, i: usize) {
        let m = &self.menus[i];
        match m.kind {
            MenuKind::Main => { self.state.menu_pos = state::XY { x: m.pos.x, y: m.pos.y }; self.dirty(); }
            MenuKind::Torn => { let (path, pos) = (m.path.clone(), m.pos); if let Some(t) = self.state.torn_menus.iter_mut().find(|t| t.path == path) { t.x = pos.x; t.y = pos.y; } self.dirty(); }
            _ => {}
        }
    }
    fn close_torn(&mut self, id: u64) {
        self.close_children_of(id);
        let Some(i) = self.menu_index(id) else { return };
        let path = self.menus[i].path.clone();
        self.menus.remove(i);
        self.state.torn_menus.retain(|t| t.path != path);
        self.dirty();
    }
    fn popup_menu(&mut self, p: Pt) {
        self.close_submenus();
        let w = MenuInst::width(&self.fonts, &MAIN_MENU, "Workspace");
        let id = self.next_menu_id();
        self.menus.push(MenuInst { id, title: "Workspace".into(), items: &MAIN_MENU, path: vec![], pos: p, w, kind: MenuKind::Popup, open_item: None });
    }
    /// Topmost menu part under `p`.
    fn menu_hit(&self, p: Pt) -> Option<(usize, MenuPart)> {
        self.menus.iter().enumerate().rev().find_map(|(i, m)| m.hit(p).map(|part| (i, part)))
    }
    pub fn act(&mut self, a: Act) {
        match a {
            Act::None | Act::Disabled => {}
            Act::InfoPanel => self.show_win(WinKind::Info),
            Act::Open => { let sel = self.fv_deep_selection(); self.open_paths(&sel); }
            Act::NewFolder => { let dir = self.fv_current_dir(); self.spawn(move || Job::NewFolder(fs::new_folder(dir))); }
            Act::Duplicate => { let paths = self.fv_selected_paths(); if !paths.is_empty() { self.run_fs(format!("duplicated {} item(s)", paths.len()), move || fs::duplicate_paths(paths).map(|_| ())); } }
            Act::Destroy => {
                let sel = self.fv_deep_selection();
                if sel.is_empty() { return; }
                let what = if sel.len() == 1 { format!("“{}”", sel[0].name) } else { format!("{} items", sel.len()) };
                self.show_alert(&format!("Destroy {what}?"), "Destroy permanently deletes the file. Continue?", &["Cancel", "Destroy"], Pending::Destroy(sel.iter().map(|e| e.path.clone()).collect()));
            }
            Act::EmptyRecycler => self.show_alert("Are you sure you want to empty the Recycler?", "Its contents will be permanently deleted.", &["Cancel", "Empty"], Pending::EmptyTrash),
            Act::Copy => { self.clipboard = self.fv_selected_paths(); let n = self.clipboard.len(); self.log(format!("copied {n} path(s)")); }
            Act::Paste => { let (paths, dir) = (self.clipboard.clone(), self.fv_current_dir()); if !paths.is_empty() { self.run_fs(format!("pasted {} item(s) into {dir}", paths.len()), move || fs::copy_paths(paths, dir)); } }
            Act::SelectAll => self.fv_select_all(),
            Act::CheckDisks => self.fv_refresh(),
            Act::ViewBrowser => {}
            Act::Scale1 => self.set_zoom(1.0),
            Act::Scale15 => self.set_zoom(1.5),
            Act::Scale2 => self.set_zoom(2.0),
            Act::ShowHidden => { self.state.show_hidden = !self.state.show_hidden; self.dirty(); self.fv_refresh(); }
            Act::Backdrop => { self.state.backdrop = !self.state.backdrop; self.dirty(); }
            Act::ShowDock => { self.state.dock_visible = !self.state.dock_visible; self.dirty(); self.dock_request_icons(); }
            Act::ShowMiniwindows => { self.state.miniwindows_visible = !self.state.miniwindows_visible; self.dirty(); }
            Act::ShowRecycler => { self.state.recycler_visible = !self.state.recycler_visible; self.dirty(); }
            Act::RecyclerWin => self.show_win(WinKind::Recycler),
            Act::Inspector => self.show_win(WinKind::Inspector),
            Act::ConsoleWin => self.show_win(WinKind::Console),
            Act::Mandelbrot => self.show_win(WinKind::Mandelbrot),
            Act::ShellWin => self.show_win(WinKind::Shell),
            Act::FileViewerWin => self.show_win(WinKind::FileViewer),
            Act::ArrangeFront => {
                let mut order: Vec<WinKind> = self.wins.iter().filter(|w| w.shown() && w.kind != WinKind::Alert).map(|w| w.kind).collect();
                order.sort_by_key(|k| self.win(*k).z);
                for k in order { self.activate_win(k); }
            }
            Act::Miniaturize => { if let Some(k) = self.key { self.miniaturize(k); } }
            Act::CloseWin => { if let Some(k) = self.key { self.close_win(k); } }
            Act::Hide => { if cfg!(target_os = "macos") { self.minimize = true; } }
            Act::Quit => { self.save_now(); self.quit = true; }
        }
        self.redraw = true;
    }
    fn set_zoom(&mut self, z: f32) { self.zoom = z; self.state.scale = z; self.dirty(); }

    // ---- events -------------------------------------------------------------------------------
    pub fn handle(&mut self, ev: Ev) {
        self.redraw = true;
        match ev {
            Ev::Focus(f) => { self.focused = f; if f { self.fv_refresh(); } }
            Ev::MouseMove(p) => { self.mouse = p; self.mouse_move(p); }
            Ev::MouseDown(p, b, m) => { self.mouse = p; self.mouse_down(p, b, m); }
            Ev::MouseUp(p, b, m) => { self.mouse = p; self.mouse_up(p, b, m); }
            Ev::Wheel(p, dx, dy) => self.wheel(p, dx, dy),
            Ev::Key(k, m) => self.key_down(k, m),
        }
    }

    fn mouse_down(&mut self, p: Pt, b: Button, mods: Mods) {
        if self.capture.is_some() { return; }
        if self.cfg.demo.is_some() { return; }
        if self.alert.is_some() {
            if let Some(i) = self.alert_buttons().iter().position(|r| r.contains(p)) { self.capture = Some(Capture::Press(Btn::AlertBtn(i))); }
            else if self.win(WinKind::Alert).title_bar().contains(p) { let r = self.win(WinKind::Alert).r; self.capture = Some(Capture::WinMove { k: WinKind::Alert, grab: pt(p.x - r.x, p.y - r.y) }); }
            return;
        }
        // menus first (they float above everything)
        if let Some((m, part)) = self.menu_hit(p) {
            if b != Button::Left { return; }
            match part {
                MenuPart::Close => self.capture = Some(Capture::Press(Btn::MenuClose(self.menus[m].id))),
                MenuPart::Title => { let pos = self.menus[m].pos; let t = self.menus.remove(m); let m = t.id; self.menus.push(t); self.capture = Some(Capture::MenuDrag { m, grab: pt(p.x - pos.x, p.y - pos.y), moved: false }); }
                MenuPart::Item(i) => {
                    let it = &self.menus[m].items[i];
                    let (disabled, _) = self.item_state(it);
                    if disabled { return; }
                    if it.sub.is_some() { self.open_submenu(self.menus[m].id, i); } else { self.capture = Some(Capture::Press(Btn::MenuItem(self.menus[m].id, i))); }
                }
            }
            return;
        }
        self.close_submenus();
        if b == Button::Right {
            if self.tile_hit(p).is_none() && self.win_hit(p).is_none() { self.popup_menu(p); }
            return;
        }
        if b != Button::Left { return; }
        if let Some(t) = self.tile_hit(p) {
            match t {
                Btn::Tile(TileId::App(i)) => self.capture = Some(Capture::TilePress { idx: i, start: p, dx: 0, dy: 0 }),
                Btn::Miniwin(k) => {
                    let now = self.now;
                    if self.dock.last_mini_click.is_some_and(|(kk, t)| kk == k && now.duration_since(t) < Duration::from_millis(400)) { self.dock.last_mini_click = None; self.restore(k); }
                    else { self.dock.last_mini_click = Some((k, now)); }
                }
                other => self.capture = Some(Capture::Press(other)),
            }
            return;
        }
        if let Some((k, part)) = self.win_hit(p) {
            self.make_key(k);
            match part {
                WinPart::MiniBtn => self.capture = Some(Capture::Press(Btn::WinMini(k))),
                WinPart::CloseBtn => self.capture = Some(Capture::Press(Btn::WinClose(k))),
                WinPart::Title => { let r = self.win(k).r; self.capture = Some(Capture::WinMove { k, grab: pt(p.x - r.x, p.y - r.y) }); }
                WinPart::Resize(region) => self.capture = Some(Capture::WinResize { k, start: p, orig: self.win(k).r, region }),
                WinPart::Content => self.content_down(k, p, mods),
            }
        }
    }
    fn content_down(&mut self, k: WinKind, p: Pt, mods: Mods) {
        match k {
            WinKind::FileViewer => self.fv_mouse_down(p, mods),
            WinKind::Inspector => self.insp_mouse_down(p),
            WinKind::Console => { let sc = self.console_scroller(); self.scroller_down(ScrollId::Console, sc, p); }
            WinKind::Recycler => self.rec_mouse_down(p),
            WinKind::Mandelbrot => self.mandel_mouse_down(p),
            WinKind::Shell => self.shell_mouse_down(p),
            _ => {}
        }
    }
    /// Shared scroller press handling: arrows are pressables, the knob is a drag, the track pages.
    pub fn scroller_down(&mut self, id: ScrollId, sc: Scroller, p: Pt) -> bool {
        let Some(hit) = sc.hit(p) else { return false };
        match hit {
            ScrollHit::ArrowA => self.capture = Some(Capture::Press(Btn::ScrollArrow(id, -1))),
            ScrollHit::ArrowB => self.capture = Some(Capture::Press(Btn::ScrollArrow(id, 1))),
            ScrollHit::Knob => self.capture = Some(Capture::Knob { id, start: p, start_pos: sc.pos }),
            ScrollHit::PageBack => self.scroll_by(id, -sc.visible),
            ScrollHit::PageFwd => self.scroll_by(id, sc.visible),
        }
        true
    }
    pub fn scroll_by(&mut self, id: ScrollId, d: i32) { let cur = self.scroll_pos(id); self.set_scroll(id, cur + d); }
    /// Current (clamped) scroll position.
    pub fn scroll_pos(&self, id: ScrollId) -> i32 {
        match id {
            ScrollId::Col(cid) => self.fv_col_scroller(cid).map_or(0, |s| s.pos),
            ScrollId::Browser => self.fv_layout().hscroll.pos,
            ScrollId::Console => self.console_scroller().pos,
            ScrollId::InspText => self.insp.scroll,
            ScrollId::Recycler => self.rec.scroll,
            ScrollId::Shell => self.shell_scroller().pos,
        }
    }
    pub fn set_scroll(&mut self, id: ScrollId, v: i32) {
        let max = self.scroller_for(id).map_or(0, |s| s.max_pos());
        let v = v.clamp(0, max);
        match id {
            ScrollId::Col(cid) => { if let Some(c) = self.fv.cols.iter_mut().find(|c| c.id == cid) { c.scroll = v; } }
            ScrollId::Browser => self.fv.hscroll = v,
            ScrollId::Console => { self.console_scroll = v; self.console_stick = v >= max; }
            ScrollId::InspText => self.insp.scroll = v,
            ScrollId::Recycler => self.rec.scroll = v,
            ScrollId::Shell => self.shell_set_scroll(v),
        }
        self.redraw = true;
    }
    fn scroller_for(&self, id: ScrollId) -> Option<Scroller> {
        match id {
            ScrollId::Col(cid) => self.fv_col_scroller(cid),
            ScrollId::Browser => Some(self.fv_layout().hscroll),
            ScrollId::Console => Some(self.console_scroller()),
            ScrollId::InspText => self.insp_text_scroller(),
            ScrollId::Recycler => Some(self.rec_scroller()),
            ScrollId::Shell => Some(self.shell_scroller()),
        }
    }

    fn mouse_move(&mut self, p: Pt) {
        let Some(cap) = self.capture.take() else { return };
        let (sw, sh) = (self.w, self.h);
        match cap {
            Capture::WinMove { k, grab } => { let w = self.win_mut(k); w.r.x = p.x - grab.x; w.r.y = p.y - grab.y; w.clamp(sw, sh); self.capture = Some(Capture::WinMove { k, grab }); }
            Capture::WinResize { k, start, orig, region } => {
                let (dx, dy) = (p.x - start.x, p.y - start.y);
                let (min_w, min_h) = { let w = self.win(k); (w.min_w, w.min_h) };
                let w = self.win_mut(k);
                let mut r = orig;
                if region < 0 { r.w = (orig.w - dx).max(min_w); r.x = orig.x + (orig.w - r.w); }
                if region > 0 { r.w = (orig.w + dx).max(min_w); }
                r.h = (orig.h + dy).max(min_h);
                w.r = r;
                if k == WinKind::Shell { self.shell_resize(); }
                self.capture = Some(Capture::WinResize { k, start, orig, region });
            }
            Capture::MenuDrag { m, grab, moved } => {
                let Some(i) = self.menu_index(m) else { return };
                let moved_now = moved || (p.x - grab.x - self.menus[i].pos.x).abs() + (p.y - grab.y - self.menus[i].pos.y).abs() > 4;
                if moved_now {
                    if matches!(self.menus[i].kind, MenuKind::Sub { .. }) { self.tear_off(i); }
                    self.menus[i].pos = pt(p.x - grab.x, p.y - grab.y);
                    self.reattach(i);
                }
                self.capture = Some(Capture::MenuDrag { m, grab, moved: moved_now });
            }
            Capture::Knob { id, start, start_pos } => {
                if let Some(sc) = self.scroller_for(id) {
                    let d = if sc.vertical { p.y - start.y } else { p.x - start.x };
                    let v = sc.drag_pos(start_pos, d);
                    self.set_scroll(id, v);
                }
                self.capture = Some(Capture::Knob { id, start, start_pos });
            }
            Capture::CellPress { col, idx, start } => {
                if (p.x - start.x).abs().max((p.y - start.y).abs()) > 4 { self.fv.last_click = None; self.fv_start_drag(col); }
                else { self.capture = Some(Capture::CellPress { col, idx, start }); }
            }
            Capture::ShelfPress { idx, start } => {
                if (p.x - start.x).abs().max((p.y - start.y).abs()) > 4 {
                    let path = self.state.shelf[idx].clone();
                    let icon = self.fv_path_icon(&path);
                    self.capture = Some(Capture::FileDrag { paths: vec![path], icon, from_shelf: Some(idx) });
                } else { self.capture = Some(Capture::ShelfPress { idx, start }); }
            }
            Capture::TilePress { idx, start, .. } => {
                let (dx, dy) = (p.x - start.x, p.y - start.y);
                self.capture = Some(Capture::TilePress { idx, start, dx, dy });
            }
            Capture::MandelDrag { start } => { self.mandel_drag(start, p); self.capture = Some(Capture::MandelDrag { start }); }
            other => self.capture = Some(other),
        }
        self.redraw = true;
    }

    fn mouse_up(&mut self, p: Pt, _b: Button, mods: Mods) {
        let Some(cap) = self.capture.take() else { return };
        match cap {
            Capture::Press(btn) => { if self.btn_hit(p) == Some(btn) { self.activate(btn); } }
            Capture::WinMove { k, .. } => self.sync_win_state(k),
            Capture::WinResize { k, .. } => self.sync_win_state(k),
            Capture::MenuDrag { m, moved, .. } => { if moved { if let Some(i) = self.menu_index(m) { self.menu_moved(i); } } }
            Capture::CellPress { .. } | Capture::Knob { .. } => {}
            Capture::ShelfPress { idx, .. } => { if self.btn_hit(p) == Some(Btn::Shelf(idx)) { self.activate(Btn::Shelf(idx)); } }
            Capture::FileDrag { paths, from_shelf, .. } => self.drop(p, paths, from_shelf, mods),
            Capture::TilePress { idx, dx, dy, .. } => self.dock_tile_release(idx, dx, dy),
            Capture::MandelDrag { start } => self.mandel_release(start, p, mods),
        }
        self.redraw = true;
    }

    /// Pressable under `p`, honoring z-order (alert → menus → tiles → windows).
    pub fn btn_hit(&self, p: Pt) -> Option<Btn> {
        if self.alert.is_some() { return self.alert_buttons().iter().position(|r| r.contains(p)).map(Btn::AlertBtn); }
        if let Some((m, part)) = self.menu_hit(p) {
            let m = self.menus[m].id;
            return match part { MenuPart::Close => Some(Btn::MenuClose(m)), MenuPart::Item(i) => Some(Btn::MenuItem(m, i)), MenuPart::Title => None };
        }
        if let Some(t) = self.tile_hit(p) { return Some(t); }
        let (k, part) = self.win_hit(p)?;
        match part {
            WinPart::MiniBtn => Some(Btn::WinMini(k)),
            WinPart::CloseBtn => Some(Btn::WinClose(k)),
            WinPart::Content => match k {
                WinKind::FileViewer => self.fv_btn_hit(p),
                WinKind::Inspector => self.insp_btn_hit(p),
                WinKind::Console => self.console_scroller().hit(p).and_then(|h| match h { ScrollHit::ArrowA => Some(Btn::ScrollArrow(ScrollId::Console, -1)), ScrollHit::ArrowB => Some(Btn::ScrollArrow(ScrollId::Console, 1)), _ => None }),
                WinKind::Recycler => self.rec_btn_hit(p),
                WinKind::Mandelbrot => self.mandel_btn_hit(p),
                WinKind::Shell => self.shell_btn_hit(p),
                _ => None,
            },
            _ => None,
        }
    }
    /// Pressed-state visual: only while the mouse is still over the pressed control.
    pub fn pressed(&self) -> Option<Btn> {
        match &self.capture { Some(Capture::Press(b)) if self.btn_hit(self.mouse) == Some(*b) => Some(*b), _ => None }
    }
    fn activate(&mut self, btn: Btn) {
        match btn {
            Btn::WinMini(k) => self.miniaturize(k),
            Btn::WinClose(k) => self.close_win(k),
            Btn::MenuClose(m) => self.close_torn(m),
            Btn::MenuItem(m, i) => {
                let Some(mi) = self.menu_index(m) else { return };
                let act = self.menus[mi].items[i].act;
                let popup = self.root_of(m) == Some(MenuKind::Popup);
                self.close_submenus();
                if popup { self.menus.retain(|x| x.kind != MenuKind::Popup); }
                self.act(act);
            }
            Btn::AlertBtn(i) => self.alert_button(i),
            Btn::Tile(t) => self.dock_tile_click(t),
            Btn::Miniwin(_) => {}
            Btn::ScrollArrow(id, d) => { let step = match id { ScrollId::Browser => crate::fileviewer::COL_PITCH, ScrollId::Col(_) => crate::fileviewer::CELL_H, ScrollId::Shell => 1, _ => 18 }; self.scroll_by(id, d as i32 * step); }
            Btn::Shelf(i) => { if let Some(p) = self.state.shelf.get(i).cloned() { self.fv_navigate(&p); } }
            Btn::PathItem(i) => { if let Some(p) = self.fv_path_components().get(i).cloned() { self.fv_navigate(&p); } }
            Btn::InspPopup => self.insp.popup_open = !self.insp.popup_open,
            Btn::InspRow(i) => self.insp_set_mode(i),
            Btn::InspCompute => self.insp_compute(),
            Btn::RecBtn => self.rec_button(),
            Btn::Mandel(b) => self.mandel_btn(b),
            Btn::Cell(..) => {}
        }
    }
    fn root_of(&self, mut m: u64) -> Option<MenuKind> {
        // Bound traversal so even an invalid graph cannot hang the event loop.
        for _ in 0..self.menus.len() {
            match self.menus[self.menu_index(m)?].kind { MenuKind::Sub { parent, .. } => m = parent, k => return Some(k) }
        }
        None
    }

    fn wheel(&mut self, p: Pt, dx: f32, dy: f32) {
        if self.alert.is_some() { return; }
        let Some((k, WinPart::Content)) = self.win_hit(p) else { return };
        let (dx, dy) = (dx.round() as i32, dy.round() as i32);
        match k {
            WinKind::FileViewer => self.fv_wheel(p, dx, dy),
            WinKind::Console => self.scroll_by(ScrollId::Console, dy),
            WinKind::Inspector => self.scroll_by(ScrollId::InspText, dy),
            WinKind::Recycler => self.scroll_by(ScrollId::Recycler, dy),
            WinKind::Shell => self.shell_wheel(dy),
            _ => {}
        }
    }

    fn key_down(&mut self, k: Key, mods: Mods) {
        if self.alert.is_some() {
            let n = self.alert.as_ref().map_or(0, |a| a.buttons.len());
            match k { Key::Enter => self.alert_button(n.saturating_sub(1)), Key::Escape => self.alert_button(0), _ => {} }
            return;
        }
        // the Shell gets every key (on Windows including Ctrl combinations); macOS Cmd shortcuts still reach the menus
        if self.key == Some(WinKind::Shell) && self.win(WinKind::Shell).shown() && !(cfg!(target_os = "macos") && mods.cmd) { self.shell_key(k, mods); return; }
        if mods.cmd {
            if let Key::Char(c) = k {
                if let Some(it) = find_key(&MAIN_MENU, c) {
                    let (disabled, _) = self.item_state(it);
                    if !disabled { self.close_submenus(); self.act(it.act); }
                }
            }
            return;
        }
        if self.key == Some(WinKind::FileViewer) && self.win(WinKind::FileViewer).shown() { self.fv_key(k, mods); }
    }

    // ---- drag & drop (§7.8) --------------------------------------------------------------------
    fn drop(&mut self, p: Pt, paths: Vec<String>, from_shelf: Option<usize>, mods: Mods) {
        if let Some(i) = from_shelf {
            // dragging a Shelf item off the Shelf removes it (§7.8); it never drops anywhere else
            let lay = self.fv_layout();
            if !(self.win(WinKind::FileViewer).shown() && lay.shelf.contains(p)) && i < self.state.shelf.len() { self.state.shelf.remove(i); self.dirty(); }
            return;
        }
        if let Some(t) = self.tile_hit(p) {
            match t {
                Btn::Tile(TileId::Recycler) => self.trash_paths(paths),
                Btn::Tile(TileId::App(i)) => { let app = self.state.dock[i].clone(); for path in paths { match apps::open_with(&app, &path) { Ok(()) => self.log(format!("opened {} with {}", icons::basename(&path), icons::basename(&app))), Err(e) => self.error("Cannot open", e) } } }
                _ => {}
            }
            return;
        }
        if self.state.dock_visible && self.dock_strip().contains(p) { for path in paths.into_iter().filter(|p| icons::is_app_path(&icons::basename(p))) { self.dock_add(path); } return; }
        if let Some((WinKind::FileViewer, WinPart::Content)) = self.win_hit(p) { self.fv_drop(p, paths, mods.alt); }
    }
    pub fn dragging(&self) -> Option<(&Vec<String>, &'static str)> {
        match &self.capture { Some(Capture::FileDrag { paths, icon, .. }) => Some((paths, icon)), _ => None }
    }

    // ---- drawing ---------------------------------------------------------------------------------
    /// Every surface, back to front. In multi-window mode each becomes an OS window; headless draws them in order.
    pub fn surfaces(&self) -> Vec<SurfaceInfo> {
        let mut v = vec![];
        if self.cfg.demo.is_some() { return v; }
        let sf = |id: SurfaceId, r: Rect, level: Level, title: &str| SurfaceInfo { id, r, level, title: title.to_string() };
        if self.state.backdrop { v.push(sf(SurfaceId::Backdrop, rect(0, 0, self.w, self.h), Level::Bottom, "ReWorkspace")); }
        let mut order: Vec<&Win> = self.wins.iter().filter(|w| w.shown() && w.kind != WinKind::Alert).collect();
        order.sort_by_key(|w| w.z);
        for w in order { v.push(sf(SurfaceId::Win(w.kind), rect(w.r.x - 1, w.r.y - 1, w.r.w + 2, w.r.h + 2), Level::Normal, &w.title)); }
        if self.state.dock_visible {
            let n = 1 + self.state.dock.len().min(12) as i32;
            v.push(sf(SurfaceId::Dock, rect(self.w - 67, 0, 64, 64 * n), Level::Top, "Dock"));
        }
        if self.state.recycler_visible { v.push(sf(SurfaceId::Recycler, self.tile_rect(TileId::Recycler), Level::Top, "Recycler")); }
        v.push(sf(SurfaceId::AppTile, rect(0, self.h - 64, 64, 64), Level::Top, "Workspace"));
        if self.state.miniwindows_visible {
            for w in &self.wins { if let Some(slot) = w.mini { v.push(sf(SurfaceId::Miniwin(w.kind), miniwindow_rect(slot, self.h), Level::Top, &w.title)); } }
        }
        for m in &self.menus { let r = m.rect(); v.push(sf(SurfaceId::Menu(m.id), rect(r.x, r.y, r.w + 1, r.h), Level::Top, &m.title)); }
        if self.alert.is_some() { let r = self.win(WinKind::Alert).r; v.push(sf(SurfaceId::Win(WinKind::Alert), rect(r.x - 1, r.y - 1, r.w + 2, r.h + 2), Level::Top, "Alert")); }
        if self.dragging().is_some() { v.push(sf(SurfaceId::Ghost, rect(self.mouse.x - 24, self.mouse.y - 24, 48, 48), Level::Top, "")); }
        v
    }
    /// Draw one surface; the painter's origin is at the surface's top-left (Screen coordinates).
    pub fn draw_surface(&self, id: SurfaceId, p: &mut Painter) {
        let pressed = self.pressed();
        match id {
            SurfaceId::Backdrop => p.fill(rect(0, 0, self.w, self.h), DARK),
            SurfaceId::Win(k) => self.draw_win(p, wi(k)),
            SurfaceId::Menu(mid) => {
                if let Some(m) = self.menus.iter().find(|m| m.id == mid) {
                    let hi = match pressed { Some(Btn::MenuItem(mm, ii)) if mm == mid => Some(ii), _ => None };
                    let st = |it: &ItemDef| self.item_state(it);
                    m.draw(p, hi, pressed == Some(Btn::MenuClose(mid)), &st);
                }
            }
            SurfaceId::Dock => self.draw_dock(p),
            SurfaceId::Recycler => self.draw_recycler_tile(p),
            SurfaceId::AppTile => self.draw_apptile(p),
            SurfaceId::Miniwin(k) => { let w = self.win(k); if let Some(slot) = w.mini { draw_miniwindow(p, miniwindow_rect(slot, self.h), w.icon, &w.title); } }
            SurfaceId::Ghost => {
                if let Some((paths, icon)) = self.dragging() {
                    let r = rect(self.mouse.x - 24, self.mouse.y - 24, 48, 48);
                    p.fill(r, LIGHT); p.outline(r, BLACK);
                    p.icon(icon, r.x, r.y, 48);
                    if paths.len() > 1 {
                        let t = format!("+{}", paths.len() - 1);
                        let w = p.text_width(FontId::Regular, 10, &t) + 6;
                        let br = rect(r.right() - w, r.bottom() - 12, w, 12);
                        p.fill(br, BLACK);
                        p.text_in(FontId::Regular, 10, br, Align::Center, &t, WHITE);
                    }
                }
            }
        }
    }
    /// Headless composite: the whole Screen in one frame.
    pub fn draw(&self, p: &mut Painter) {
        p.fill(rect(0, 0, self.w, self.h), DARK);
        if self.cfg.demo.as_deref() == Some("chrome") { crate::demo::draw_demo_chrome(self, p); return; }
        for sf in self.surfaces() { if sf.id != SurfaceId::Backdrop { self.draw_surface(sf.id, p); } }
    }
    fn draw_win(&self, p: &mut Painter, i: usize) {
        let w = &self.wins[i];
        let pressed = match self.pressed() { Some(Btn::WinMini(k)) if k == w.kind => Some(WinPart::MiniBtn), Some(Btn::WinClose(k)) if k == w.kind => Some(WinPart::CloseBtn), _ => None };
        w.draw_chrome(p, self.key == Some(w.kind), pressed);
        let c = w.content();
        p.push_clip(c);
        match w.kind {
            WinKind::FileViewer => self.fv_draw(p, c),
            WinKind::Inspector => self.insp_draw(p, c),
            WinKind::Console => self.console_draw(p, c),
            WinKind::Info => self.info_draw(p, c),
            WinKind::Recycler => self.rec_draw(p, c),
            WinKind::Mandelbrot => self.mandel_draw(p, c),
            WinKind::Shell => self.shell_draw(p, c),
            WinKind::Alert => self.alert_draw(p, c),
        }
        p.pop_clip();
    }
    fn alert_draw(&self, p: &mut Painter, c: Rect) {
        let Some(a) = &self.alert else { return };
        // 1.0 layout: icon + "Alert" header, groove, message, buttons bottom right
        p.icon("alert", c.x + 10, c.y + 8, 48);
        p.text(FontId::Bold, 18, c.x + 70, c.y + 40, "Alert", BLACK);
        p.hline(c.x, c.y + 64, c.w, DARK); p.hline(c.x, c.y + 65, c.w, WHITE);
        p.text(FontId::Regular, 12, c.x + 10, c.y + 84, &a.message, BLACK);
        p.text(FontId::Regular, 12, c.x + 10, c.y + 100, &a.detail, BLACK);
        let pressed = self.pressed();
        for (i, r) in self.alert_buttons().iter().enumerate() {
            button(p, *r, &a.buttons[i], pressed == Some(Btn::AlertBtn(i)), i == a.buttons.len() - 1);
        }
    }
    fn console_scroller(&self) -> Scroller {
        let c = self.content_rect(WinKind::Console);
        let total = self.console.len() as i32 * 13 + 4;
        let visible = c.h;
        let pos = if self.console_stick { (total - visible).max(0) } else { self.console_scroll.min((total - visible).max(0)) };
        Scroller::framed(rect(c.x, c.y, SCROLL_W, c.h), true, total, visible, pos)
    }
    fn console_draw(&self, p: &mut Painter, c: Rect) {
        let sc = self.console_scroller();
        let list = rect(c.x + SCROLL_W, c.y, c.w - SCROLL_W, c.h);
        p.push_clip(list);
        let first = (sc.pos / 13).max(0) as usize;
        for (i, line) in self.console.iter().enumerate().skip(first).take((c.h / 13 + 2) as usize) {
            let y = c.y + 2 + i as i32 * 13 - sc.pos;
            p.text(FontId::Mono, 11, list.x + 4, y + 10, line, BLACK);
        }
        p.pop_clip();
        sc.draw(p, self.pressed().and_then(|b| match b { Btn::ScrollArrow(ScrollId::Console, d) => Some(if d < 0 { ScrollHit::ArrowA } else { ScrollHit::ArrowB }), _ => None }));
    }
    fn info_draw(&self, p: &mut Painter, c: Rect) {
        p.icon("workspace", c.x + (c.w - 96) / 2, c.y + 4, 96);
        let line = |p: &mut Painter, y: i32, f: FontId, s: i32, t: &str| p.text_in(f, s, rect(c.x, c.y + y, c.w, 16), Align::Center, t, BLACK);
        line(p, 102, FontId::Bold, 14, "ReWorkspace");
        line(p, 118, FontId::Regular, 12, &format!("Version {}", env!("CARGO_PKG_VERSION")));
        line(p, 134, FontId::Regular, 12, "A NeXTSTEP-style workspace.");
        line(p, 148, FontId::Regular, 12, "Not affiliated with NeXT or Apple.");
        line(p, 162, FontId::Regular, 12, &crate::backend::apps::platform());
    }
}

pub fn ts() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let local = secs as i64 + local_offset();
    let (h, m, s) = ((local / 3600).rem_euclid(24), (local / 60).rem_euclid(60), local.rem_euclid(60));
    format!("{h:02}:{m:02}:{s:02}")
}
#[cfg(unix)]
fn local_offset() -> i64 {
    // SAFETY: localtime_r with a valid tm buffer; tm_gmtoff is the UTC offset in seconds.
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        tm.tm_gmtoff as i64
    }
}
#[cfg(not(unix))]
fn local_offset() -> i64 { 0 }

#[cfg(test)]
mod tests {
    use super::*;

    fn click(app: &mut App, r: Rect) {
        let p = pt(r.x + 5, r.y + 5);
        app.handle(Ev::MouseDown(p, Button::Left, Mods::default()));
        app.handle(Ev::MouseUp(p, Button::Left, Mods::default()));
    }

    fn submenu(app: &mut App, parent: u64, label: &str) -> u64 {
        let i = app.menu_index(parent).unwrap();
        let item = app.menus[i].items.iter().position(|it| it.label == label).unwrap();
        app.open_submenu(parent, item);
        app.menus.iter().find(|m| matches!(m.kind, MenuKind::Sub { parent: p, .. } if p == parent)).unwrap().id
    }

    #[test]
    fn workspace_title_reorder_then_close_view_does_not_panic() {
        let mut app = App::test_app();
        let main = app.menus[0].id;
        submenu(&mut app, main, "View");
        let title = app.menus[app.menu_index(main).unwrap()].title_rect();
        click(&mut app, title);
        assert_eq!(app.menus.last().unwrap().id, main);
        let i = app.menu_index(main).unwrap();
        let view = app.menus[i].items.iter().position(|it| it.label == "View").unwrap();
        let r = app.menus[i].item_rect(view);
        click(&mut app, r);
        assert_eq!(app.menus.len(), 1);
        assert_eq!(app.menus[0].id, main);
        assert_eq!(app.menus[0].open_item, None);
    }

    #[test]
    fn closing_reordered_menu_removes_all_descendants() {
        let mut app = App::test_app();
        let main = app.menus[0].id;
        let view = submenu(&mut app, main, "View");
        let scale = submenu(&mut app, view, "Scale");
        // Grandchild, child, root: every recursive removal shifts its ancestors.
        app.menus.reverse();
        assert_eq!(app.root_of(scale), Some(MenuKind::Main));
        app.close_children_of(main);
        assert_eq!(app.menus.len(), 1);
        assert_eq!(app.menus[0].id, main);
    }

    #[test]
    fn raising_torn_view_keeps_scale_parent_and_dispatches_scale() {
        let mut app = App::test_app();
        let main = app.menus[0].id;
        let view = submenu(&mut app, main, "View");
        app.tear_off(app.menu_index(view).unwrap());
        let scale = submenu(&mut app, view, "Scale");
        let m = app.menu_index(main).unwrap();
        let item = app.menus[m].items.iter().position(|it| it.label == "View").unwrap();
        app.open_submenu(main, item);
        assert_eq!(app.menus.last().unwrap().id, view);
        assert_eq!(app.root_of(scale), Some(MenuKind::Torn));
        let m = app.menu_index(scale).unwrap();
        let item = app.menus[m].items.iter().position(|it| it.act == Act::Scale15).unwrap();
        app.activate(Btn::MenuItem(scale, item));
        assert_eq!(app.zoom, 1.5);
        assert!(app.menu_index(scale).is_none());
        assert!(app.menu_index(view).is_some());
    }

    #[test]
    fn popup_and_main_with_same_path_keep_distinct_ancestry() {
        let mut app = App::test_app();
        app.popup_menu(pt(400, 400));
        let popup = app.menus.last().unwrap().id;
        let view = submenu(&mut app, popup, "View");
        app.menus.reverse();
        assert_eq!(app.root_of(view), Some(MenuKind::Popup));
    }

    #[test]
    fn explicit_activations_are_queued_without_focus_feedback() {
        let mut app = App::test_app();
        app.show_win(WinKind::FileViewer);
        app.show_win(WinKind::Console);
        app.take_activations();
        app.act(Act::FileViewerWin);
        assert_eq!(app.take_activations(), [WinKind::FileViewer]);
        app.make_key(WinKind::Console);
        assert!(app.take_activations().is_empty());
        app.dock_tile_click(TileId::Workspace);
        assert_eq!(app.take_activations(), [WinKind::FileViewer]);
        app.act(Act::ArrangeFront);
        assert_eq!(app.take_activations(), [WinKind::Console, WinKind::FileViewer]);
        app.miniaturize(WinKind::FileViewer);
        app.restore(WinKind::FileViewer);
        assert_eq!(app.take_activations(), [WinKind::FileViewer]);
    }

    #[test]
    fn console_wheel_and_arrow_start_at_the_displayed_bottom() {
        let mut app = App::test_app();
        app.console = vec!["log line".into(); 100];
        app.show_win(WinKind::Console);
        let bottom = app.console_scroller().pos;
        assert!(bottom > 18);
        assert_eq!(app.console_scroll, 0);
        let c = app.content_rect(WinKind::Console);
        app.handle(Ev::Wheel(pt(c.x + 20, c.y + 20), 0.0, -18.0));
        assert_eq!(app.console_scroller().pos, bottom - 18);
        app.console_stick = true;
        app.activate(Btn::ScrollArrow(ScrollId::Console, -1));
        assert_eq!(app.console_scroller().pos, bottom - 18);
    }

    #[test]
    fn native_close_closes_windows_and_cancels_alerts_without_confirming() {
        let mut app = App::test_app();
        app.show_win(WinKind::FileViewer);
        app.close_surface(SurfaceId::Win(WinKind::FileViewer));
        assert!(!app.win(WinKind::FileViewer).visible);
        assert!(!app.state.windows.file_viewer.open);
        assert!(!app.quit);
        app.show_alert("Delete?", "", &["Cancel", "Delete"], Pending::Destroy(vec!["must not run".into()]));
        app.close_surface(SurfaceId::Win(WinKind::Alert));
        assert!(app.alert.is_none());
        assert!(!app.win(WinKind::Alert).visible);
        let main = app.menus[0].id;
        let view = submenu(&mut app, main, "View");
        app.tear_off(app.menu_index(view).unwrap());
        submenu(&mut app, view, "Scale");
        app.close_surface(SurfaceId::Menu(view));
        assert_eq!(app.menus.len(), 1);
        assert!(app.state.torn_menus.is_empty());
        app.cfg.demo = Some("test".into()); // Quit must not write real user state in a unit test.
        app.close_surface(SurfaceId::Menu(main));
        assert!(app.quit);
    }

    #[test]
    fn hide_is_disabled_on_platforms_without_an_implementation() {
        let mut app = App::test_app();
        let hide = MAIN_MENU.iter().find(|it| it.act == Act::Hide).unwrap();
        assert_eq!(app.item_state(hide).0, !cfg!(target_os = "macos"));
        app.handle(Ev::Key(Key::Char('h'), Mods { cmd: true, ..Mods::default() }));
        assert_eq!(app.wants_minimize(), cfg!(target_os = "macos"));
    }
}
