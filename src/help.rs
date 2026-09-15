//! The Help panel behind every application's `Info ▸ Help…`: one page of plain text per
//! application, the one whose menu the item was picked from.
use crate::app::*;
use crate::chrome::*;
use crate::geom::Rect;
use crate::paint::*;

pub const WIN_W: i32 = 420;
pub const WIN_H: i32 = 320;
const LINE_H: i32 = 15;

/// One page per application, keyed by the title its main menu carries (see `APP_MENUS`).
/// The lines are wrapped where they are written — the panel only draws them.
pub static HELP: [(&str, &str); 6] = [
    ("Workspace", "\
The File Viewer browses your real disk: click through
the columns to walk into folders, double-click a file
to open it with the host system, and drag a folder onto
the Shelf to keep it in reach.

Delete moves the selection to the Recycler; Empty
Recycler, in the File menu, empties it for good. The
Inspector shows what is selected, the Console what has
happened.

The Tools menu opens the other applications. Scale, in
the View menu, sets the size of everything on the
Screen, and Screen Backdrop puts the dark NeXT screen
behind it all."),
    ("Shell", "\
A real login shell in a terminal window, drawn in the
four grays.

Type as you would in any terminal. The scroller down the
left walks the scrollback, and resizing the window
changes the grid the shell is told about.

New Shell starts a fresh login shell in this window —
also the way back after one has exited. Clear Buffer
empties the screen and the scrollback; the prompt
returns with the next Return."),
    ("Librarian", "\
Full-text search over the bookshelves, which are the
folders sitting on the File Viewer Shelf.

Click a bookshelf to search in it, click it again to
drop it. Type a word into the Find field and press
Return: the documents come back ranked by how often the
word occurs, the first matching line beside each. The
arrow keys walk the results; a double-click, or Open
Document, opens one.

Only text documents are read, dot files are skipped, and
a search stops after 4000 documents or 300 hits."),
    ("Mandelbrot", "\
The escape-time set in four grays, after the 1.0 demo.

Click the image to zoom in on that point, Shift-click to
zoom out, or drag a rectangle around the part you want.
Reset goes back to the whole set.

Dither picks how the grays are laid down: Standard PS,
Knight's Tour, Ohlfs Mix or Error Diffusion. Deeper and
Shallower double or halve the iteration depth — deeper
takes longer and finds more detail.

Save writes Mandelbrot-N.png into your home folder."),
    ("Concurrence", "\
One outline, seen three ways.

In Outline, type to edit a topic in place. Return splits
a topic in two, Backspace at the start joins it to the
one above, Tab demotes it with its whole subtree and
Shift-Tab promotes it. Click a triangle to collapse a
topic, click the text to place the caret.

Slide draws the current topic as a 4:3 slide: every
top-level topic is a slide and its children are its
bullets. Present takes the whole Screen — Space, the
arrow keys or a click advance it, Escape ends the show.

Save writes Presentation-N.txt into your home folder."),
    ("Improv", "\
A worksheet with no A1 or B2 anywhere: the figures live
in a cube of named categories, and the formulas are
written over the item names.

Click a cell and type to enter a figure. Click the empty
line under the formulas to write one — Revenue = Units *
Price applies to every cell of that item. Sum, Avg, Min,
Max, Count, Round, Abs, Int, Sqrt and If are available;
a category name as an argument brings one figure per
item, as in Avg(Quarters).

Drag the category tiles at the top between the row zone
and the column zone to pivot the view. New Row and New
Column, in the Item menu, add an item to the innermost
category of that zone; click a header twice to rename
it."),
];

impl App {
    /// `Help…`: the page of the application whose menu it came from.
    pub fn help_open(&mut self) {
        let title = self.main_menu().0;
        self.help = HELP.iter().position(|(n, _)| *n == title).unwrap_or(0);
        self.show_win(WinKind::Help);
    }
    pub fn help_draw(&self, p: &mut Painter, c: Rect) {
        let (name, body) = HELP[self.help];
        p.text(FontId::Bold, 14, c.x + 14, c.y + 24, name, BLACK);
        p.hline(c.x + 14, c.y + 32, c.w - 28, DARK);
        p.hline(c.x + 14, c.y + 33, c.w - 28, WHITE);
        for (i, line) in body.lines().enumerate() {
            p.text(FontId::Regular, 12, c.x + 14, c.y + 52 + i as i32 * LINE_H, line, BLACK);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every application's menu has a Help item, and it opens that application's page.
    #[test]
    fn every_menu_leads_to_its_own_page() {
        let mut app = App::test_app();
        for (name, body) in HELP {
            assert!(body.lines().count() as i32 * LINE_H + 64 <= WIN_H, "{name}'s page fits the panel");
            assert!(body.lines().all(|l| l.chars().count() <= 58), "{name}'s page is wrapped to the panel width");
            // Liberation Sans has no geometric shapes: a ▸ would draw as a hollow box.
            assert!(body.chars().all(|c| c.is_ascii() || "—’“”×".contains(c)), "{name}'s page keeps to glyphs the font has");
        }
        for &(kind, name, _) in &APP_MENUS {
            app.win_mut(kind).visible = true; // not show_win: the Shell's would spawn a PTY
            app.make_key(kind);
            app.act(Act::Help);
            assert_eq!(HELP[app.help].0, name, "{name} ▸ Info ▸ Help… is {name}'s page");
            assert!(app.win(WinKind::Help).shown());
            let main = app.menus.iter().find(|m| m.kind == MenuKind::Main).unwrap();
            assert_eq!(main.title, name, "the Help panel is {name}'s, so the menu stays {name}'s");
            app.close_win(WinKind::Help);
            app.win_mut(kind).visible = false;
        }
        app.act(Act::Help); // no application window is up any more
        assert_eq!(HELP[app.help].0, "Workspace");
    }
}
