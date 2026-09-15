mod app;
mod backend;
mod chrome;
mod concur;
mod demo;
mod dock;
mod fileviewer;
mod geom;
mod icons;
mod improv;
mod inspector;
mod mandel;
mod paint;
mod shell;

use app::{App, Button, Config, Ev, Level, Mods, SurfaceId, SurfaceInfo};
use chrome::WinKind;
use geom::{pt, rect, Pt, Rect};
use paint::{Fonts, Frame, Icons, Painter};
use std::collections::{HashMap, HashSet};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{CustomCursor, Window, WindowId, WindowLevel};

pub use app::Job;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned());
    let cfg = Config {
        demo: flag("--demo"),
        home_override: flag("--home"),
        config_override: flag("--config"),
        scale_override: flag("--scale").and_then(|v| v.trim_end_matches('x').parse().ok()),
    };
    if let Some(out) = flag("--render-icon") { // used by scripts/bundle-macos.sh to build the .icns
        let px: u32 = flag("--px").and_then(|v| v.parse().ok()).unwrap_or(512);
        let (w, h, rgba) = Icons::load().rgba("workspace", px);
        std::fs::write(&out, paint::encode_png_rgba(w, h, &rgba)).expect("write icon");
        return;
    }
    if let Some(script) = flag("--headless") {
        let out = flag("--out").unwrap_or_else(|| ".".into());
        headless(cfg, &script, &out);
        return;
    }
    let mut builder = EventLoop::<Job>::with_user_event();
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        builder.with_default_menu(false); // Cmd-Q must reach our Quit (which saves state), not NSApp.terminate
    }
    let event_loop = builder.build().expect("event loop");
    let proxy = event_loop.create_proxy();
    let fonts = Rc::new(Fonts::load());
    let icons = Rc::new(Icons::load());
    let app = App::new(cfg, Arc::new(move |job| { let _ = proxy.send_event(job); }), fonts.clone());
    let mut h = Handler {
        app, fonts, icons, surfs: HashMap::new(), by_id: HashMap::new(), visible: rect(0, 0, 1120, 832), dpi: 1.0,
        mods: Mods::default(), cursor: pt(0, 0), cursors: Vec::new(), focused: HashSet::new(), any_focus: false, zoom: 1.0, started: false,
    };
    event_loop.run_app(&mut h).expect("run");
}

/// One real, undecorated OS window backing one Screen element.
struct Surf {
    window: Rc<Window>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
    id: SurfaceId,
    rect: Rect,   // model rect (Screen coordinates) it was last synced to
    cursor: usize,
}

struct Handler {
    app: App,
    fonts: Rc<Fonts>,
    icons: Rc<Icons>,
    surfs: HashMap<WindowId, Surf>,
    by_id: HashMap<SurfaceId, WindowId>,
    visible: Rect, // usable area of the primary screen in logical points, top-left origin
    dpi: f32,
    mods: Mods,
    cursor: Pt,
    cursors: Vec<CustomCursor>,
    focused: HashSet<WindowId>,
    any_focus: bool,
    zoom: f32,
    started: bool,
}

impl Handler {
    fn scale(&self) -> f32 { self.dpi * self.zoom }
    fn os_pos(&self, r: Rect) -> LogicalPosition<f64> {
        let z = self.zoom as f64;
        LogicalPosition::new(self.visible.x as f64 + r.x as f64 * z, self.visible.y as f64 + r.y as f64 * z)
    }
    fn os_size(&self, r: Rect) -> LogicalSize<f64> { let z = self.zoom as f64; LogicalSize::new((r.w as f64 * z).max(1.0), (r.h as f64 * z).max(1.0)) }
    fn screen_size(&mut self) { let z = self.zoom; self.app.resize((self.visible.w as f32 / z).floor() as i32, (self.visible.h as f32 / z).floor() as i32); }
    fn to_model(&self, wid: WindowId, pos: PhysicalPosition<f64>) -> Pt {
        let s = self.scale();
        let r = self.surfs.get(&wid).map_or(rect(0, 0, 0, 0), |su| su.rect);
        pt(r.x + (pos.x as f32 / s).floor() as i32, r.y + (pos.y as f32 / s).floor() as i32)
    }

    fn make_cursors(&mut self, el: &ActiveEventLoop) {
        let px = (16.0 * self.dpi).round() as u32;
        for (name, hx, hy) in [("arrow", 0, 0), ("resize-nesw", 8, 8), ("resize-ns", 8, 8), ("resize-nwse", 8, 8)] {
            let (cw, ch, rgba) = self.icons.rgba(name, px);
            let k = cw as f32 / 16.0;
            if let Ok(src) = CustomCursor::from_rgba(rgba, cw as u16, ch as u16, (hx as f32 * k) as u16, (hy as f32 * k) as u16) {
                self.cursors.push(el.create_custom_cursor(src));
            }
        }
    }

    /// Make the set of OS windows match the model's surfaces (create, move/resize, destroy).
    fn sync(&mut self, el: &ActiveEventLoop, force_geometry: bool) {
        let desired = self.app.surfaces();
        let wanted: HashSet<SurfaceId> = desired.iter().map(|d| d.id).collect();
        let stale: Vec<SurfaceId> = self.by_id.keys().filter(|id| !wanted.contains(id)).copied().collect();
        for id in stale {
            if let Some(wid) = self.by_id.remove(&id) { self.surfs.remove(&wid); self.focused.remove(&wid); }
        }
        for d in desired {
            match self.by_id.get(&d.id).copied() {
                Some(wid) => {
                    let (pos, size) = (self.os_pos(d.r), self.os_size(d.r));
                    let su = self.surfs.get_mut(&wid).unwrap();
                    if force_geometry || (su.rect.x, su.rect.y) != (d.r.x, d.r.y) { su.window.set_outer_position(pos); }
                    if force_geometry || (su.rect.w, su.rect.h) != (d.r.w, d.r.h) { let _ = su.window.request_inner_size(size); su.window.request_redraw(); }
                    su.rect = d.r;
                }
                None => self.create(el, d),
            }
        }
        for k in self.app.take_activations() {
            if let Some(su) = self.by_id.get(&SurfaceId::Win(k)).and_then(|wid| self.surfs.get(wid)) {
                su.window.set_minimized(false);
                su.window.focus_window();
            }
        }
    }

    fn create(&mut self, el: &ActiveEventLoop, d: SurfaceInfo) {
        let level = match d.level { Level::Bottom => WindowLevel::AlwaysOnBottom, Level::Normal => WindowLevel::Normal, Level::Top => WindowLevel::AlwaysOnTop };
        #[allow(unused_mut)]
        let mut attrs = Window::default_attributes()
            .with_title(&d.title).with_decorations(false).with_resizable(false)
            .with_position(self.os_pos(d.r)).with_inner_size(self.os_size(d.r)).with_window_level(level).with_visible(true);
        #[cfg(target_os = "macos")]
        {
            use winit::platform::macos::WindowAttributesExtMacOS;
            attrs = attrs.with_has_shadow(false); // NeXT windows cast no shadows
        }
        let window = Rc::new(el.create_window(attrs).expect("window"));
        self.dpi = window.scale_factor() as f32;
        let ctx = softbuffer::Context::new(window.clone()).expect("softbuffer context");
        let surface = softbuffer::Surface::new(&ctx, window.clone()).expect("surface");
        if self.cursors.is_empty() { self.make_cursors(el); }
        if let Some(c) = self.cursors.first() { window.set_cursor(c.clone()); }
        if d.id == SurfaceId::Win(WinKind::Alert) { window.focus_window(); }
        let wid = window.id();
        self.by_id.insert(d.id, wid);
        self.surfs.insert(wid, Surf { window, surface, id: d.id, rect: d.r, cursor: 0 });
    }

    fn redraw(&mut self, wid: WindowId) {
        let s = self.scale();
        let Some(su) = self.surfs.get_mut(&wid) else { return };
        let size = su.window.inner_size();
        let (Some(pw), Some(ph)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else { return };
        su.surface.resize(pw, ph).expect("resize");
        let mut buf = su.surface.buffer_mut().expect("buffer");
        {
            let frame = Frame { w: size.width, h: size.height, px: &mut buf };
            let mut p = Painter::new(frame, s, &self.fonts, &self.icons);
            p.set_origin(su.rect.x, su.rect.y);
            self.app.draw_surface(su.id, &mut p);
        }
        buf.present().expect("present");
    }

    fn after(&mut self, el: &ActiveEventLoop) {
        if self.app.quit { el.exit(); return; }
        if self.app.wants_minimize() { hide_app(); }
        let zoom_changed = (self.app.zoom - self.zoom).abs() > 0.001;
        if zoom_changed { self.zoom = self.app.zoom; self.screen_size(); }
        self.sync(el, zoom_changed);
        if self.app.take_redraw() { for su in self.surfs.values() { su.window.request_redraw(); } }
    }
}

impl ApplicationHandler<Job> for Handler {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.started { return; }
        self.started = true;
        self.visible = visible_area(el);
        self.dpi = el.primary_monitor().map_or(1.0, |m| m.scale_factor() as f32);
        self.zoom = self.app.zoom;
        self.screen_size();
        self.app.start();
        self.after(el);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, wid: WindowId, event: WindowEvent) {
        let Some(sid) = self.surfs.get(&wid).map(|s| s.id) else { return };
        if matches!(event, WindowEvent::RedrawRequested) { self.redraw(wid); return; }
        // Input can wake WaitUntil early or arrive after other work in the same batch.
        self.app.tick(Instant::now());
        match event {
            WindowEvent::CloseRequested => self.app.close_surface(sid),
            WindowEvent::Moved(pos) => {
                // the OS may constrain a window (screen edges); follow it in the model
                let z = self.zoom;
                let x = ((pos.x as f32 / self.dpi - self.visible.x as f32) / z).round() as i32;
                let y = ((pos.y as f32 / self.dpi - self.visible.y as f32) / z).round() as i32;
                if let Some(su) = self.surfs.get_mut(&wid) {
                    if (x - su.rect.x).abs() > 1 || (y - su.rect.y).abs() > 1 { su.rect.x = x; su.rect.y = y; self.app.surface_moved(sid, x, y); }
                }
            }
            WindowEvent::Focused(f) => {
                if f { self.focused.insert(wid); if let SurfaceId::Win(k) = sid { self.app.make_key(k); } } else { self.focused.remove(&wid); }
            }
            WindowEvent::ModifiersChanged(m) => {
                let s: ModifiersState = m.state();
                self.mods = Mods { shift: s.shift_key(), alt: s.alt_key(), ctrl: s.control_key(), cmd: if cfg!(target_os = "macos") { s.super_key() } else { s.control_key() } };
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = self.to_model(wid, position);
                self.app.handle(Ev::MouseMove(self.cursor));
                let want = self.app.cursor_kind();
                if let Some(su) = self.surfs.get_mut(&wid) {
                    if want != su.cursor && want < self.cursors.len() { su.window.set_cursor(self.cursors[want].clone()); su.cursor = want; }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let b = match button { MouseButton::Left => Button::Left, MouseButton::Right => Button::Right, _ => Button::Other };
                if state == ElementState::Pressed {
                    if let SurfaceId::Win(k) = sid { self.app.raise(k); }
                    self.app.handle(Ev::MouseDown(self.cursor, b, self.mods));
                } else { self.app.handle(Ev::MouseUp(self.cursor, b, self.mods)); }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta { MouseScrollDelta::LineDelta(x, y) => (-x * 18.0, -y * 18.0), MouseScrollDelta::PixelDelta(p) => (-p.x as f32 / self.scale(), -p.y as f32 / self.scale()) };
                self.app.handle(Ev::Wheel(self.cursor, dx, dy));
            }
            WindowEvent::KeyboardInput { event: KeyEvent { logical_key, state: ElementState::Pressed, .. }, .. } => {
                let key = match logical_key {
                    Key::Named(n) => match n {
                        NamedKey::ArrowUp => app::Key::Up, NamedKey::ArrowDown => app::Key::Down, NamedKey::ArrowLeft => app::Key::Left, NamedKey::ArrowRight => app::Key::Right,
                        NamedKey::Enter => app::Key::Enter, NamedKey::Escape => app::Key::Escape, NamedKey::Backspace => app::Key::Backspace, NamedKey::Delete => app::Key::Delete,
                        NamedKey::Tab => app::Key::Tab, NamedKey::Home => app::Key::Home, NamedKey::End => app::Key::End, NamedKey::PageUp => app::Key::PageUp, NamedKey::PageDown => app::Key::PageDown,
                        NamedKey::Space => app::Key::Char(' '), _ => return,
                    },
                    Key::Character(s) => match s.chars().next() { Some(c) => app::Key::Char(c), None => return },
                    _ => return,
                };
                self.app.handle(Ev::Key(key, self.mods));
            }
            _ => {}
        }
        self.after(el);
    }

    fn user_event(&mut self, el: &ActiveEventLoop, job: Job) { self.app.tick(Instant::now()); self.app.on_job(job); self.after(el); }

    fn new_events(&mut self, el: &ActiveEventLoop, _cause: StartCause) {
        self.app.tick(Instant::now());
        if self.started { self.after(el); }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        // app-level focus: any of our windows focused? (drives the 5 s refresh and closes menus on deactivation, §9.7)
        let any = !self.focused.is_empty();
        if any != self.any_focus {
            self.any_focus = any;
            self.app.handle(Ev::Focus(any));
            if !any && self.app.has_open_menus() { self.app.close_open_menus(); }
            self.after(el);
        }
        el.set_control_flow(match self.app.next_deadline() { Some(t) => ControlFlow::WaitUntil(t), None => ControlFlow::Wait });
    }
}

/// Usable area of the primary screen (menu bar and Dock excluded on macOS), logical points, top-left origin.
fn visible_area(el: &ActiveEventLoop) -> Rect {
    #[cfg(target_os = "macos")]
    {
        if let Some(mtm) = objc2_foundation::MainThreadMarker::new() {
            let screens = objc2_app_kit::NSScreen::screens(mtm);
            if let Some(scr) = screens.first() {
                let (f, v) = (scr.frame(), scr.visibleFrame());
                let top = f.size.height - (v.origin.y + v.size.height);
                return rect(v.origin.x as i32, top as i32, v.size.width as i32, v.size.height as i32);
            }
        }
    }
    let (w, h, s) = el.primary_monitor().map(|m| (m.size().width, m.size().height, m.scale_factor())).unwrap_or((1120, 832, 1.0));
    rect(0, 0, (w as f64 / s) as i32, (h as f64 / s) as i32)
}

/// `Hide` (§7.3): hide the whole application like NeXT did; the macOS Dock icon brings it back.
fn hide_app() {
    #[cfg(target_os = "macos")]
    if let Some(mtm) = objc2_foundation::MainThreadMarker::new() { objc2_app_kit::NSApplication::sharedApplication(mtm).hide(None); }
}

/// Headless mode: replay a script of events and dump PNG screenshots of the composited Screen.
fn headless(cfg: Config, script: &str, out_dir: &str) {
    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel::<Job>();
    let fonts = Rc::new(Fonts::load());
    let icons = Icons::load();
    let tx = std::sync::Mutex::new(tx);
    let mut app = App::new(cfg, Arc::new(move |job| { let _ = tx.lock().unwrap().send(job); }), fonts.clone());
    let (mut w, mut h) = (1120i32, 832i32);
    let mut zoom_scale = app.zoom; // honours --scale
    let mut mods = Mods::default();
    let mut cursor = pt(0, 0);
    app.resize((w as f32 / zoom_scale) as i32, (h as f32 / zoom_scale) as i32);
    app.start();
    let text = std::fs::read_to_string(script).expect("script");
    let settle = |app: &mut App, ms: u64| {
        let end = Instant::now() + std::time::Duration::from_millis(ms);
        loop {
            while let Ok(j) = rx.try_recv() { app.on_job(j); }
            app.tick(Instant::now());
            if Instant::now() >= end { break; }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    };
    settle(&mut app, 300);
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        let n = |i: usize| parts[i].parse::<i32>().unwrap();
        match parts[0] {
            "size" => { w = n(1); h = n(2); app.resize((w as f32 / zoom_scale) as i32, (h as f32 / zoom_scale) as i32); }
            "move" => { cursor = pt(n(1), n(2)); app.handle(Ev::MouseMove(cursor)); }
            "down" => app.handle(Ev::MouseDown(cursor, Button::Left, mods)),
            "up" => app.handle(Ev::MouseUp(cursor, Button::Left, mods)),
            "click" | "rclick" => {
                cursor = pt(n(1), n(2));
                let b = if parts[0] == "rclick" { Button::Right } else { Button::Left };
                app.handle(Ev::MouseMove(cursor)); app.handle(Ev::MouseDown(cursor, b, mods)); app.handle(Ev::MouseUp(cursor, b, mods));
            }
            "dblclick" => { cursor = pt(n(1), n(2)); for _ in 0..2 { app.handle(Ev::MouseDown(cursor, Button::Left, mods)); app.handle(Ev::MouseUp(cursor, Button::Left, mods)); } }
            "drag" => {
                let (a, b) = (pt(n(1), n(2)), pt(n(3), n(4)));
                app.handle(Ev::MouseMove(a)); app.handle(Ev::MouseDown(a, Button::Left, mods));
                for i in 1..=8 { let p = pt(a.x + (b.x - a.x) * i / 8, a.y + (b.y - a.y) * i / 8); app.handle(Ev::MouseMove(p)); }
                cursor = b;
            }
            "dragsel" | "dragcell" => {
                let r: Vec<i32> = app.debug_query("selrect").split_whitespace().filter_map(|v| v.parse().ok()).collect();
                let t: Vec<i32> = if parts[0] == "dragcell" {
                    let c: Vec<i32> = app.debug_query(&format!("cellrect {}", parts[1])).split_whitespace().filter_map(|v| v.parse().ok()).collect();
                    if c.len() == 4 { vec![c[0] + 40, c[1] + 8] } else { vec![] }
                } else { vec![n(1), n(2)] };
                if r.len() == 4 && t.len() == 2 {
                    let (a, b) = (pt(r[0] + 40, r[1] + 8), pt(t[0], t[1]));
                    app.handle(Ev::MouseMove(a)); app.handle(Ev::MouseDown(a, Button::Left, mods));
                    for i in 1..=8 { let p = pt(a.x + (b.x - a.x) * i / 8, a.y + (b.y - a.y) * i / 8); app.handle(Ev::MouseMove(p)); }
                    cursor = b;
                } else { eprintln!("{}: no selection/cell", parts[0]); }
            }
            "drop" => app.handle(Ev::MouseUp(cursor, Button::Left, mods)),
            "mods" => { mods = Mods { shift: parts.contains(&"shift"), alt: parts.contains(&"alt"), ctrl: parts.contains(&"ctrl"), cmd: parts.contains(&"cmd") }; }
            "key" => {
                let k = match parts[1] { "up" => app::Key::Up, "down" => app::Key::Down, "left" => app::Key::Left, "right" => app::Key::Right, "enter" => app::Key::Enter, "esc" => app::Key::Escape, "del" => app::Key::Delete, "backspace" => app::Key::Backspace, "tab" => app::Key::Tab, "home" => app::Key::Home, "end" => app::Key::End, "pgup" => app::Key::PageUp, "pgdn" => app::Key::PageDown, s => app::Key::Char(s.chars().next().unwrap()) };
                app.handle(Ev::Key(k, mods));
            }
            "type" => for c in parts[1..].join(" ").chars() { app.handle(Ev::Key(app::Key::Char(c), mods)); },
            "wheel" => app.handle(Ev::Wheel(cursor, n(1) as f32, n(2) as f32)),
            "wait" => settle(&mut app, n(1) as u64),
            "zoom" => { zoom_scale = parts[1].parse().unwrap_or(1.0); app.zoom = zoom_scale; app.resize((w as f32 / zoom_scale) as i32, (h as f32 / zoom_scale) as i32); }
            "shot" => {
                settle(&mut app, 50);
                let (pw, ph) = (w as u32, h as u32);
                let mut px = vec![0u32; (pw * ph) as usize];
                { let mut p = Painter::new(Frame { w: pw, h: ph, px: &mut px }, zoom_scale, &fonts, &icons); app.draw(&mut p); }
                let path = format!("{out_dir}/{}", parts[1]);
                std::fs::write(&path, paint::encode_png(pw, ph, &px)).expect("write png");
                eprintln!("shot {path}");
            }
            "log" => eprintln!("{}", app.console_tail(n(1) as usize).join("\n")),
            "eval" => eprintln!("{}", app.debug_query(&parts[1..].join(" "))),
            other => eprintln!("headless: unknown command {other}"),
        }
        settle(&mut app, 0);
    }
}
