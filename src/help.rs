//! `Info ▸ Help…` — one panel, a page per application, as every NeXTSTEP application had. The page
//! is the key window's, picked when the panel opens (opening it makes the panel itself key).
use crate::app::*;
use crate::chrome::*;
use crate::geom::Rect;
use crate::paint::*;

const PAD: i32 = 14;
const LINE: i32 = 15;
const KEY_W: i32 = 116; // the key column; the widest label here is “Shift-Tab”
pub const HELP_W: i32 = 430;

/// A line is either `key<tab>what it does` or a sentence; an empty one is a blank line. Keep the
/// text to what the window does not already show — this is help, not the README.
const PAGES: [(WinKind, &str, &str); 10] = [
    (WinKind::FileViewer, "Workspace", "\
The Workspace browses the file system in columns.

Arrows\tmove in the columns
Return\topen the selection
Delete\tmove it to the Recycler
letters\ttype-ahead to a name
double-click\topen a file with its host application

Drag a folder to the Shelf to keep it there. Press
the right mouse button on the Screen for the main
menu; drag a submenu away by its title to tear it
off, and it stays until you close it."),
    (WinKind::Inspector, "Inspector", "\
Inspector, in the Tools menu, describes the File
Viewer's selection: name, size, kind, dates and
permissions.

Compute\tmeasure a folder, which walks it
Contents\tshows text files and PNG images

It follows the selection, so click another file and
the Inspector follows along."),
    (WinKind::Console, "Console", "\
The Console logs what the Workspace does: copies,
deletions, searches, saved files and errors.

It is the place to look when something did not
happen — a file that would not copy says why here."),
    (WinKind::Recycler, "Recycler", "\
Files deleted in the File Viewer land here, in the
system trash. Show Recycler Tile, in the View menu,
puts the tile on the Screen; drop files on it to
delete them.

Empty Recycler\tin the File menu; it always asks first

This is the real trash folder, so anything emptied
is gone for good."),
    (WinKind::Shell, "Shell", "\
Shell, in the Tools menu, runs your login shell.
Keys go straight to it, including Control
combinations.

New Shell\tstart a fresh login shell in the window
Clear Buffer\tempty the screen and the scrollback

Resize the window to change the character grid; the
title follows what the shell reports. Scrollback is
on the scroller down the left edge.

On macOS, Cmd shortcuts still reach the menus."),
    (WinKind::Librarian, "Digital Librarian", "\
Librarian, in the Tools menu, searches the folders
on the Shelf for a word, and ranks the documents by
how often they use it.

click a book\tsearch that folder, or unpick it
Return\tsearch
Up/Down\twalk the hits
double-click\topen the document
Escape\tclear the word

It reads the files as it goes rather than keeping an
index, and stops after 4000 documents or 300 hits."),
    (WinKind::Mandelbrot, "Mandelbrot", "\
The set drawn in the four NeXT grays, with a choice
of dithering.

click\tzoom in about that point
shift-click\tzoom out
drag\tzoom to the rectangle
Depth\thow long a point is given to escape
Reset\tback to the whole set
Save\twrite a PNG to your home folder

Deeper zooms need more depth, or the inside of the
set floods the picture."),
    (WinKind::Improv, "Improv", "\
A worksheet of named categories — no A1 or B2. The
tiles at the top are the categories; drag one
between the row zone and the column zone to pivot.

click a cell\tthen type a figure
click a header\tpick that row or column
click it again\trename it
click a formula\tedit it
empty line\twrite another formula

Formulas are written over the names, as in
Revenue = Units * Price, and apply to every cell of
that item. New Row and New Column, in the Item menu,
add an item to the innermost category of that zone."),
    (WinKind::Concurrence, "Concurrence", "\
One document three ways: an outline, its slides, and
the show. Every top-level topic is a slide and its
children are the bullets.

Tab\tdemote it, as Move Right does
Shift-Tab\tpromote it, as Move Left does
Return\tsplit the topic at the caret
Backspace\tjoin it with the one above
click a triangle\tcollapse or expand a topic
Space\tnext slide in the show
Escape\tend the show

Save writes the outline as Presentation-N.txt in
your home folder; it also lives in state.json."),
    (WinKind::Preferences, "Preferences", "\
Preferences, in the Info menu of every application,
keeps the settings of the Screen. The icons along
the top are the modules; click one for its pane.

Display\tthe scale everything is drawn at
Workspace\tthe Dock, the tiles and the backdrop
Expert\thidden files, and where settings live

Drag the scale knob and let go: text and icons are
drawn again at the new size, from a quarter step to
four times. The View menu has the same commands, so
a switch here and its menu item always agree."),
];

fn page(k: WinKind) -> (&'static str, &'static str) {
    let (_, title, body) = PAGES.iter().find(|(w, ..)| *w == k).unwrap_or(&PAGES[0]);
    (title, body)
}
/// The panel is as tall as its page, so it never needs a scroller.
fn height(body: &str) -> i32 { TITLE_H + PAD * 2 + body.lines().count() as i32 * LINE }

impl App {
    /// `Info ▸ Help…` from any application's menu: the page is the one whose menu it came from, so
    /// picking it twice keeps the page (the panel itself is nobody's application).
    pub fn show_help(&mut self) {
        self.help = self.menu_owner().filter(|k| PAGES.iter().any(|(w, ..)| w == k)).unwrap_or(WinKind::FileViewer);
        let (title, body) = page(self.help);
        let (title, h) = (format!("{title} Help"), height(body));
        let win = self.win_mut(WinKind::Help);
        win.title = title;
        win.r.h = h;
        self.show_win(WinKind::Help);
    }
    pub fn help_draw(&self, p: &mut Painter, c: Rect) {
        let (_, body) = page(self.help);
        let mut y = c.y + PAD + 11;
        for line in body.lines() {
            match line.split_once('\t') {
                Some((k, t)) => {
                    p.text(FontId::Bold, 12, c.x + PAD, y, k, BLACK);
                    p.text(FontId::Regular, 12, c.x + PAD + KEY_W, y, &p.ellipsize(FontId::Regular, 12, t, c.w - PAD * 2 - KEY_W), BLACK);
                }
                None => { p.text(FontId::Regular, 12, c.x + PAD, y, line, BLACK); }
            }
            y += LINE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_opens_on_the_key_window_and_fits_every_page() {
        let mut a = App::test_app();
        a.show_win(WinKind::Shell);
        a.act(Act::Help);
        assert_eq!(a.help, WinKind::Shell, "the page is the window that was key, not the panel");
        assert_eq!(a.win(WinKind::Help).title, "Shell Help");
        assert!(a.win(WinKind::Help).shown());

        a.act(Act::Help); // the panel is key now, but it is the Shell's, and so is the menu
        assert_eq!(a.help, WinKind::Shell);
        a.close_win(WinKind::Shell);
        a.act(Act::Help);
        assert_eq!(a.win(WinKind::Help).title, "Workspace Help");

        let fonts = a.fonts.clone();
        for (k, _, body) in PAGES {
            assert!(height(body) <= a.h, "the {k:?} page is taller than the Screen");
            for line in body.lines() {
                let (key, text) = line.split_once('\t').unwrap_or(("", line));
                for c in line.chars().filter(|c| *c != '\t') {
                    assert!(fonts.has_glyph(FontId::Regular, c) && fonts.has_glyph(FontId::Bold, c), "no glyph for “{c}” in the {k:?} page");
                }
                assert!(fonts.width_px(FontId::Bold, 12, key) as i32 <= KEY_W, "“{key}” does not fit the key column");
                assert!(fonts.width_px(FontId::Regular, 12, text) as i32 <= HELP_W - PAD * 2 - if key.is_empty() { 0 } else { KEY_W }, "“{text}” is too wide");
            }
        }
    }
}
