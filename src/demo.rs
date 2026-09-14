//! `--demo chrome`: bevels, fonts, icons and truncation on one screen, for look review.
use crate::app::App;
use crate::geom::rect;
use crate::paint::*;

pub fn draw_demo_chrome(_app: &App, p: &mut Painter) {
    let btn = rect(40, 40, 100, 24);
    p.raised(btn);
    p.text_in(FontId::Regular, 12, btn, Align::Center, "Raised Button", BLACK);
    let well = rect(160, 40, 110, 24);
    p.sunken(well);
    p.text_in(FontId::Regular, 12, well.inset(4), Align::Left, "Sunken well", BLACK);
    let mut x = 40;
    for (f, size, s) in [(FontId::Regular, 12, "Regular 12 px"), (FontId::Bold, 12, "Bold 12 px"), (FontId::Regular, 10, "Small 10 px"), (FontId::Mono, 11, "Mono 11 px"), (FontId::Bold, 14, "Bold 14 px")] {
        x += p.text(f, size, x, 90, s, BLACK) + 16;
    }
    p.raised(rect(40, 110, 720, 150));
    let names = ["alert", "application", "computer", "drive", "drive-net", "file-archive", "file-audio", "file-code", "file-generic", "file-image", "file-pdf", "file-text", "file-video", "folder", "folder-open", "home", "miniwindow", "recycler-empty", "recycler-full", "workspace"];
    for (i, n) in names.iter().enumerate() { p.icon(n, 48 + (i as i32 % 12) * 56, 118 + (i as i32 / 12) * 60, 48); }
    for (i, n) in ["symlink-badge", "arrow", "resize-ns", "resize-nesw", "resize-nwse"].iter().enumerate() {
        p.icon(n, 500 + i as i32 * 24, 190, 16);
    }
    p.raised(rect(40, 270, 720, 20));
    for (i, n) in ["folder", "home", "computer", "file-text", "application", "recycler-empty"].iter().enumerate() { p.icon(n, 48 + i as i32 * 24, 272, 16); }
    p.text(FontId::Regular, 12, 40, 310, &p.ellipsize_mid(FontId::Regular, 12, "a very long file name that needs truncating.txt", 120), BLACK);
    p.text(FontId::Regular, 12, 200, 310, &p.ellipsize(FontId::Regular, 12, "Applications and more text", 80), BLACK);
}
