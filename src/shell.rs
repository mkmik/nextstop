//! Shell — a terminal window (NeXTSTEP 1.0 called it Shell) on top of `alacritty_terminal`:
//! the crate owns the PTY, the escape-sequence parser and the cell grid; we draw the grid in four grays.
use crate::app::*;
use crate::chrome::*;
use crate::geom::{rect, Pt, Rect};
use crate::paint::*;
use alacritty_terminal::event::{Event, EventListener, Notify, OnResize, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, Msg, Notifier};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{point_to_viewport, test::TermSize, Config, Term, TermMode};
use alacritty_terminal::tty;
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor, Rgb};
use std::collections::HashMap;
use std::sync::Arc;

pub const CELL_W: i32 = 7;   // Liberation Mono 12 px advance (7.2) rounded
pub const CELL_H: i32 = 14;
pub const PAD: i32 = 4;

/// Forwards terminal events to the UI thread as jobs.
#[derive(Clone)]
pub struct Listener { post: Arc<dyn Fn(Job) + Send + Sync> }
impl EventListener for Listener {
    fn send_event(&self, ev: Event) {
        let job = match ev {
            Event::Wakeup | Event::CursorBlinkingChange | Event::MouseCursorDirty | Event::Bell => Job::TermWake,
            Event::Title(t) => Job::TermTitle(t),
            Event::ResetTitle => Job::TermTitle(String::new()),
            Event::PtyWrite(s) => Job::TermWrite(s),
            Event::Exit | Event::ChildExit(_) => Job::TermExit,
            Event::ColorRequest(i, fmt) => Job::TermWrite(fmt(if i == 257 { Rgb { r: 255, g: 255, b: 255 } } else { Rgb { r: 0, g: 0, b: 0 } })),
            Event::TextAreaSizeRequest(_) | Event::ClipboardStore(..) | Event::ClipboardLoad(..) => return,
        };
        (self.post)(job);
    }
}

#[derive(Default)]
pub struct Shell {
    pub term: Option<Arc<FairMutex<Term<Listener>>>>,
    notifier: Option<Notifier>,
    pub cols: usize,
    pub rows: usize,
    pub title: String,
    pub exited: bool,
}

fn quantize(l: f32) -> u32 { if l < 0.25 { BLACK } else if l < 0.6 { DARK } else if l < 0.9 { LIGHT } else { WHITE } }
fn lum(r: u8, g: u8, b: u8) -> f32 { (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) / 255.0 }
/// Map a terminal color onto the four grays; chromatic text becomes dark gray so it stays legible on white.
fn gray_of(c: Color) -> u32 {
    use NamedColor::*;
    match c {
        Color::Named(n) => match n {
            Foreground | BrightForeground | Black => BLACK,
            Background => WHITE,
            BrightBlack | DimForeground | DimBlack | Cursor => DARK,
            White | BrightWhite | DimWhite => LIGHT,
            Red | Green | Yellow | Blue | Magenta | Cyan | BrightRed | BrightGreen | BrightYellow | BrightBlue | BrightMagenta | BrightCyan
            | DimRed | DimGreen | DimYellow | DimBlue | DimMagenta | DimCyan => DARK,
        },
        Color::Spec(rgb) => quantize(lum(rgb.r, rgb.g, rgb.b)),
        Color::Indexed(i) => match i {
            0 => BLACK, 8 => DARK, 7 | 15 => LIGHT, 1..=6 | 9..=14 => DARK,
            16..=231 => { let i = i - 16; let (r, g, b) = (i / 36, (i / 6) % 6, i % 6); let v = |x: u8| if x == 0 { 0 } else { 55 + 40 * x }; quantize(lum(v(r), v(g), v(b))) }
            232..=255 => quantize((8 + 10 * (i as u32 - 232)) as f32 / 255.0),
        },
    }
}

impl App {
    fn shell_area(&self) -> Rect { let c = self.content_rect(WinKind::Shell); rect(c.x + SCROLL_W, c.y, c.w - SCROLL_W, c.h) }
    fn shell_dims(&self) -> (usize, usize) {
        let a = self.shell_area();
        (((a.w - 2 * PAD) / CELL_W).max(20) as usize, ((a.h - 2 * PAD) / CELL_H).max(5) as usize)
    }
    fn window_size(cols: usize, rows: usize) -> WindowSize { WindowSize { num_lines: rows as u16, num_cols: cols as u16, cell_width: CELL_W as u16, cell_height: CELL_H as u16 } }

    /// Spawn the login shell in a PTY sized to the window (no-op if already running).
    pub fn shell_start(&mut self) {
        if self.shell.term.is_some() { return; }
        let (cols, rows) = self.shell_dims();
        let listener = Listener { post: self.post.clone() };
        let term = Arc::new(FairMutex::new(Term::new(Config::default(), &TermSize::new(cols, rows), listener.clone())));
        let mut env = HashMap::new();
        env.insert("TERM".to_string(), "xterm-256color".to_string());
        let opts = tty::Options { shell: None, working_directory: Some(self.home.clone().into()), drain_on_exit: false, env };
        let pty = match tty::new(&opts, Self::window_size(cols, rows), 0) { Ok(p) => p, Err(e) => { self.error("Cannot start a shell", e.to_string()); return; } };
        let ev = match EventLoop::new(term.clone(), listener, pty, false, false) { Ok(e) => e, Err(e) => { self.error("Cannot start a shell", e.to_string()); return; } };
        let notifier = Notifier(ev.channel());
        ev.spawn();
        self.shell = Shell { term: Some(term), notifier: Some(notifier), cols, rows, title: "Shell".into(), exited: false };
        self.win_mut(WinKind::Shell).title = "Shell".into();
        self.log(format!("shell started ({cols}×{rows})"));
    }
    /// Stop the shell (window closed): hang up the PTY and forget the terminal.
    pub fn shell_stop(&mut self) {
        if let Some(n) = self.shell.notifier.take() { let _ = n.0.send(Msg::Shutdown); }
        self.shell.term = None;
        self.shell.exited = false;
    }
    /// Window resized: fit the grid.
    pub fn shell_resize(&mut self) {
        let (cols, rows) = self.shell_dims();
        if (cols, rows) == (self.shell.cols, self.shell.rows) { return; }
        self.shell.cols = cols; self.shell.rows = rows;
        if let Some(t) = &self.shell.term { t.lock().resize(TermSize::new(cols, rows)); }
        if let Some(n) = &mut self.shell.notifier { n.on_resize(Self::window_size(cols, rows)); }
    }
    pub fn shell_write(&mut self, s: String) { if let Some(n) = &self.shell.notifier { n.notify(s.into_bytes()); } }
    pub fn shell_title(&mut self, t: String) {
        self.shell.title = if t.is_empty() { "Shell".into() } else { format!("Shell — {t}") };
        let t = self.shell.title.clone();
        self.win_mut(WinKind::Shell).title = t;
    }
    pub fn shell_exited(&mut self) {
        if self.shell.exited || self.shell.term.is_none() { return; }
        self.shell.exited = true;
        self.win_mut(WinKind::Shell).title = "Shell — exited".into();
        self.log("shell exited".into());
    }

    /// Keyboard → bytes for the PTY (xterm conventions).
    pub fn shell_key(&mut self, k: Key, mods: Mods) {
        if self.shell.exited { self.shell_stop(); self.close_win(WinKind::Shell); return; }
        let app_cursor = self.shell.term.as_ref().is_some_and(|t| t.lock().mode().contains(TermMode::APP_CURSOR));
        let arrow = |c: char| if app_cursor { format!("\x1bO{c}") } else { format!("\x1b[{c}") };
        let bytes: Vec<u8> = match k {
            Key::Char(c) => {
                if mods.ctrl && c.is_ascii() {
                    let u = c.to_ascii_uppercase() as u8;
                    if (b'@'..=b'_').contains(&u) { vec![u & 0x1f] } else if c == ' ' { vec![0] } else { c.to_string().into_bytes() }
                } else if mods.alt { let mut v = vec![0x1b]; v.extend(c.to_string().into_bytes()); v }
                else { c.to_string().into_bytes() }
            }
            Key::Enter => b"\r".to_vec(),
            Key::Backspace => b"\x7f".to_vec(),
            Key::Tab => b"\t".to_vec(),
            Key::Escape => b"\x1b".to_vec(),
            Key::Delete => b"\x1b[3~".to_vec(),
            Key::Up => arrow('A').into_bytes(), Key::Down => arrow('B').into_bytes(), Key::Right => arrow('C').into_bytes(), Key::Left => arrow('D').into_bytes(),
            Key::Home => b"\x1b[H".to_vec(), Key::End => b"\x1b[F".to_vec(),
            Key::PageUp => b"\x1b[5~".to_vec(), Key::PageDown => b"\x1b[6~".to_vec(),
        };
        if let Some(t) = &self.shell.term { t.lock().scroll_display(Scroll::Bottom); }
        if let Some(n) = &self.shell.notifier { n.notify(bytes); }
    }
    pub fn shell_wheel(&mut self, dy: i32) {
        let lines = -(dy / CELL_H).clamp(-40, 40);
        if lines != 0 { if let Some(t) = &self.shell.term { t.lock().scroll_display(Scroll::Delta(lines)); } }
    }
    /// Scrollback as a scroller: units are lines, position 0 = oldest history.
    pub fn shell_scroller(&self) -> Scroller {
        let c = self.content_rect(WinKind::Shell);
        let (hist, off, rows) = self.shell.term.as_ref().map_or((0, 0, self.shell.rows), |t| { let t = t.lock(); (t.grid().history_size(), t.grid().display_offset(), t.grid().screen_lines()) });
        Scroller::framed(rect(c.x, c.y, SCROLL_W, c.h), true, (hist + rows) as i32, rows as i32, (hist - off) as i32)
    }
    pub fn shell_set_scroll(&mut self, v: i32) {
        let Some(t) = &self.shell.term else { return };
        let mut t = t.lock();
        let (hist, off) = (t.grid().history_size() as i32, t.grid().display_offset() as i32);
        let target = (hist - v.clamp(0, hist)) - off;
        if target != 0 { t.scroll_display(Scroll::Delta(target)); }
    }
    pub fn shell_btn_hit(&self, p: Pt) -> Option<Btn> {
        match self.shell_scroller().hit(p)? { ScrollHit::ArrowA => Some(Btn::ScrollArrow(ScrollId::Shell, -1)), ScrollHit::ArrowB => Some(Btn::ScrollArrow(ScrollId::Shell, 1)), _ => None }
    }
    pub fn shell_mouse_down(&mut self, p: Pt) { let sc = self.shell_scroller(); self.scroller_down(ScrollId::Shell, sc, p); }

    pub fn shell_draw(&self, p: &mut Painter, _c: Rect) {
        let a = self.shell_area();
        p.fill(a, WHITE);
        let pressed = self.pressed();
        let pr = match pressed { Some(Btn::ScrollArrow(ScrollId::Shell, d)) => Some(if d < 0 { ScrollHit::ArrowA } else { ScrollHit::ArrowB }), _ => None };
        self.shell_scroller().draw(p, pr);
        let Some(term) = &self.shell.term else { return };
        let term = term.lock();
        let content = term.renderable_content();
        let off = content.display_offset;
        let (rows, cols) = (self.shell.rows as i32, self.shell.cols as i32);
        let (ox, oy) = (a.x + PAD, a.y + PAD);
        p.push_clip(rect(ox, oy, cols * CELL_W, rows * CELL_H));
        for ind in content.display_iter {
            let Some(vp) = point_to_viewport(off, ind.point) else { continue };
            let (line, col) = (vp.line as i32, vp.column.0 as i32);
            if line >= rows || col >= cols { continue; }
            let cell = ind.cell;
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) { continue; }
            let (mut fg, mut bg) = (gray_of(cell.fg), gray_of(cell.bg));
            if cell.flags.contains(Flags::INVERSE) { std::mem::swap(&mut fg, &mut bg); }
            if cell.flags.contains(Flags::DIM) && fg == BLACK { fg = DARK; }
            let r = rect(ox + col * CELL_W, oy + line * CELL_H, CELL_W, CELL_H);
            if bg != WHITE { p.fill(r, bg); }
            if cell.c != ' ' && !cell.flags.contains(Flags::HIDDEN) {
                let s = cell.c.to_string();
                p.text(FontId::Mono, 12, r.x, r.y + 11, &s, fg);
                if cell.flags.contains(Flags::BOLD) { p.text(FontId::Mono, 12, r.x + 1, r.y + 11, &s, fg); }
            }
        }
        // cursor: block when key, hollow when not
        if let Some(vp) = point_to_viewport(off, content.cursor.point) {
            let (line, col) = (vp.line as i32, vp.column.0 as i32);
            if line < rows && col < cols && content.cursor.shape != CursorShape::Hidden && !self.shell.exited {
                let r = rect(ox + col * CELL_W, oy + line * CELL_H, CELL_W, CELL_H);
                let key = self.key == Some(WinKind::Shell);
                match (content.cursor.shape, key) {
                    (CursorShape::Underline, _) => p.fill(rect(r.x, r.bottom() - 2, r.w, 2), BLACK),
                    (CursorShape::Beam, _) => p.fill(rect(r.x, r.y, 1, r.h), BLACK),
                    (_, false) | (CursorShape::HollowBlock, _) => p.outline(r, BLACK),
                    _ => {
                        p.fill(r, BLACK);
                        let ch = term.grid()[content.cursor.point].c;
                        if ch != ' ' { p.text(FontId::Mono, 12, r.x, r.y + 11, &ch.to_string(), WHITE); }
                    }
                }
            }
        }
        p.pop_clip();
        if self.shell.exited { p.text(FontId::Mono, 12, ox, a.bottom() - 6, "[process exited — press any key to close]", DARK); }
    }
}
