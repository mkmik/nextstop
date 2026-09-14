mod app;
mod backend;
mod chrome;
mod demo;
mod dock;
mod fileviewer;
mod geom;
mod icons;
mod inspector;
mod paint;

use app::{App, Button, Config, Ev, Mods};
use geom::{pt, Pt};
use paint::{Fonts, Frame, Icons, Painter};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{CustomCursor, Window, WindowId};

/// Result of a background job (directory listing, icon extraction…), delivered to the App.
pub use app::Job;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned());
    let cfg = Config {
        no_anim: args.iter().any(|a| a == "--no-anim"),
        demo: flag("--demo"),
        home_override: flag("--home"),
        config_override: flag("--config"),
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
    let event_loop = EventLoop::<Job>::with_user_event().build().expect("event loop");
    let proxy = event_loop.create_proxy();
    let fonts = Rc::new(Fonts::load());
    let icons = Rc::new(Icons::load());
    let app = App::new(cfg, Arc::new(move |job| { let _ = proxy.send_event(job); }), fonts.clone());
    let mut h = Handler { window: None, surface: None, app, fonts, icons, mods: Mods::default(), cursor: pt(0, 0), dpi: 1.0, cursors: Vec::new(), cur_cursor: usize::MAX, zoom: 1 };
    event_loop.run_app(&mut h).expect("run");
}

struct Handler {
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    app: App,
    fonts: Rc<Fonts>,
    icons: Rc<Icons>,
    mods: Mods,
    cursor: Pt,
    dpi: f32,
    cursors: Vec<CustomCursor>,
    cur_cursor: usize,
    zoom: u8,
}

impl Handler {
    fn scale(&self) -> f32 { self.dpi * self.app.zoom as f32 }
    fn logical(&self, x: f64, y: f64) -> Pt { let s = self.scale(); pt((x as f32 / s).floor() as i32, (y as f32 / s).floor() as i32) }
    fn relayout(&mut self) {
        let Some(w) = &self.window else { return };
        let size = w.inner_size();
        let s = self.scale();
        self.app.resize((size.width as f32 / s).floor() as i32, (size.height as f32 / s).floor() as i32);
    }
    fn redraw(&mut self) {
        let s = self.scale();
        let (Some(w), Some(surface)) = (&self.window, &mut self.surface) else { return };
        let size = w.inner_size();
        let (Some(pw), Some(ph)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else { return };
        surface.resize(pw, ph).expect("resize");
        let mut buf = surface.buffer_mut().expect("buffer");
        {
            let frame = Frame { w: size.width, h: size.height, px: &mut buf };
            let mut p = Painter::new(frame, s, &self.fonts, &self.icons);
            self.app.draw(&mut p);
        }
        buf.present().expect("present");
        let want = self.app.cursor_kind();
        if want != self.cur_cursor && want < self.cursors.len() { w.set_cursor(self.cursors[want].clone()); self.cur_cursor = want; }
    }
    fn after(&mut self, el: &ActiveEventLoop) {
        if self.app.quit { el.exit(); return; }
        if self.app.zoom != self.zoom { self.zoom = self.app.zoom; self.relayout(); }
        if let Some(w) = &self.window {
            if self.app.wants_minimize() { w.set_minimized(true); }
            if self.app.take_redraw() { w.request_redraw(); }
        }
    }
}

impl ApplicationHandler<Job> for Handler {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() { return; }
        let (w, h) = self.app.initial_window_size();
        let attrs = Window::default_attributes().with_title("ReWorkspace").with_inner_size(LogicalSize::new(w as f64, h as f64)).with_min_inner_size(LogicalSize::new(800.0, 600.0));
        let window = Rc::new(el.create_window(attrs).expect("window"));
        self.dpi = window.scale_factor() as f32;
        let ctx = softbuffer::Context::new(window.clone()).expect("softbuffer context");
        self.surface = Some(softbuffer::Surface::new(&ctx, window.clone()).expect("surface"));
        // §6.5 custom cursors, rendered from our own SVGs at device resolution
        let px = (16.0 * self.dpi).round() as u32;
        for (name, hx, hy) in [("arrow", 0, 0), ("resize-nesw", 8, 8), ("resize-ns", 8, 8), ("resize-nwse", 8, 8)] {
            let (cw, ch, rgba) = self.icons.rgba(name, px);
            let k = cw as f32 / 16.0;
            if let Ok(src) = CustomCursor::from_rgba(rgba, cw as u16, ch as u16, (hx as f32 * k) as u16, (hy as f32 * k) as u16) {
                self.cursors.push(el.create_custom_cursor(src));
            }
        }
        self.window = Some(window);
        self.zoom = self.app.zoom;
        self.relayout();
        self.app.start();
        self.after(el);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => { self.app.save_now(); el.exit(); }
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::Resized(_) => { self.relayout(); self.app.request_redraw(); }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => { self.dpi = scale_factor as f32; self.relayout(); }
            WindowEvent::Focused(f) => self.app.handle(Ev::Focus(f)),
            WindowEvent::ModifiersChanged(m) => {
                let s: ModifiersState = m.state();
                self.mods = Mods { shift: s.shift_key(), alt: s.alt_key(), cmd: if cfg!(target_os = "macos") { s.super_key() } else { s.control_key() } };
            }
            WindowEvent::CursorMoved { position, .. } => { self.cursor = self.logical(position.x, position.y); self.app.handle(Ev::MouseMove(self.cursor)); }
            WindowEvent::MouseInput { state, button, .. } => {
                let b = match button { MouseButton::Left => Button::Left, MouseButton::Right => Button::Right, _ => Button::Other };
                let ev = if state == ElementState::Pressed { Ev::MouseDown(self.cursor, b, self.mods) } else { Ev::MouseUp(self.cursor, b, self.mods) };
                self.app.handle(ev);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta { MouseScrollDelta::LineDelta(x, y) => (-x * 18.0, -y * 18.0), MouseScrollDelta::PixelDelta(p) => (-p.x as f32 / self.scale(), -p.y as f32 / self.scale()) };
                self.app.handle(Ev::Wheel(self.cursor, dx, dy));
            }
            WindowEvent::KeyboardInput { event: KeyEvent { logical_key, state: ElementState::Pressed, .. }, .. } => {
                let key = match logical_key {
                    Key::Named(n) => match n {
                        NamedKey::ArrowUp => app::Key::Up, NamedKey::ArrowDown => app::Key::Down, NamedKey::ArrowLeft => app::Key::Left, NamedKey::ArrowRight => app::Key::Right,
                        NamedKey::Enter => app::Key::Enter, NamedKey::Escape => app::Key::Escape, NamedKey::Backspace | NamedKey::Delete => app::Key::Delete,
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

    fn user_event(&mut self, el: &ActiveEventLoop, job: Job) { self.app.on_job(job); self.after(el); }

    fn new_events(&mut self, el: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::ResumeTimeReached { .. } | StartCause::Poll) { self.app.tick(Instant::now()); self.after(el); }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        el.set_control_flow(match self.app.next_deadline() { Some(t) => ControlFlow::WaitUntil(t), None => ControlFlow::Wait });
    }
}

/// Headless mode: replay a script of events and dump PNG screenshots (used for look review and tests).
fn headless(cfg: Config, script: &str, out_dir: &str) {
    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel::<Job>();
    let fonts = Rc::new(Fonts::load());
    let icons = Icons::load();
    let tx = std::sync::Mutex::new(tx);
    let mut app = App::new(cfg, Arc::new(move |job| { let _ = tx.lock().unwrap().send(job); }), fonts.clone());
    let (mut w, mut h) = (1120i32, 832i32);
    let mut zoom_scale = 1.0f32;
    let mut mods = Mods::default();
    let mut cursor = pt(0, 0);
    app.resize(w, h);
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
            "dragsel" => { // drag from the selected Browser cell to (x, y)
                let r: Vec<i32> = app.debug_query("selrect").split_whitespace().filter_map(|v| v.parse().ok()).collect();
                if r.len() == 4 {
                    let (a, b) = (pt(r[0] + 40, r[1] + 9), pt(n(1), n(2)));
                    app.handle(Ev::MouseMove(a)); app.handle(Ev::MouseDown(a, Button::Left, mods));
                    for i in 1..=8 { let p = pt(a.x + (b.x - a.x) * i / 8, a.y + (b.y - a.y) * i / 8); app.handle(Ev::MouseMove(p)); }
                    cursor = b;
                } else { eprintln!("dragsel: no selection"); }
            }
            "drop" => app.handle(Ev::MouseUp(cursor, Button::Left, mods)),
            "mods" => { mods = Mods { shift: parts.contains(&"shift"), alt: parts.contains(&"alt"), cmd: parts.contains(&"cmd") }; }
            "key" => {
                let k = match parts[1] { "up" => app::Key::Up, "down" => app::Key::Down, "left" => app::Key::Left, "right" => app::Key::Right, "enter" => app::Key::Enter, "esc" => app::Key::Escape, "del" => app::Key::Delete, s => app::Key::Char(s.chars().next().unwrap()) };
                app.handle(Ev::Key(k, mods));
            }
            "type" => for c in parts[1..].join(" ").chars() { app.handle(Ev::Key(app::Key::Char(c), mods)); },
            "wheel" => app.handle(Ev::Wheel(cursor, n(1) as f32, n(2) as f32)),
            "wait" => settle(&mut app, n(1) as u64),
            "zoom" => { zoom_scale = n(1) as f32; app.zoom = n(1) as u8; app.resize((w as f32 / zoom_scale) as i32, (h as f32 / zoom_scale) as i32); }
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
