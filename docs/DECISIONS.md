# Decisions / deviations from the PRD

One line per deviation, with the reason.

## Native rewrite (v0.2)

- **Stack change requested by the owner**: the Tauri + HTML/TS frontend (kept in git history as tag `v0.1.0-tauri`) was replaced by a native Rust program. The PRD ruled out egui/iced because bevels and text metrics are painful there; instead the UI is drawn by our own software painter (`src/paint.rs`) into a `softbuffer` framebuffer in a `winit` window. Text is rasterized from the bundled Liberation TTFs with `fontdue`, icons from our SVGs with `resvg`. No webview, no JavaScript, no GPU.
- The PRD's repository layout, `?demo=` pages and Playwright notes are web-specific. Equivalents: `--demo chrome` renders the bevel/font/icon test screen; `--headless <script> --out <dir>` replays mouse/keyboard events and writes PNG screenshots (this is how every screenshot in `docs/screenshots` was made and how the app is tested without a display).
- `--home <dir>` and `--config <dir>` override the home and config directories so tests run against a throwaway tree.
- Milestone screenshots `m0…m6` belonged to the web version and were removed; the native screenshots are named by content.
- macOS packaging is `scripts/bundle-macos.sh` (plain `.app` + `hdiutil` DMG, ad-hoc codesign); Windows ships the bare `reworkspace.exe`, which is fully portable (no WebView2 needed any more). The optional NSIS installer and an embedded `.exe` icon were not done.
- Miniaturize animation shrinks a gray window outline (120 ms) instead of scaling the live window contents.

## Real windows instead of one container (v0.3)

- The owner asked for the app to work without being contained in another window. Every Screen element is now its own **undecorated OS window** (winit `with_decorations(false)`, no shadow): File Viewer, Inspector, Console, Recycler, Info, alerts, each open menu, the Dock column, the Recycler tile, the application tile and every miniwindow. The model still lives in one Screen coordinate system whose origin is the top-left of the primary display's *visible frame* (menu bar and macOS Dock excluded, via `NSScreen.visibleFrame`); each window paints its part of the Screen through the same painter with a shifted origin.
- Window levels: menus, Dock, tiles, miniwindows, alerts and the drag ghost are always-on-top (like NeXT's floating menu and Dock); NeXT windows are normal-level and interleave with other apps' windows; the optional dark **Screen Backdrop** (`View ▸ Screen Backdrop`, default off) is an always-on-bottom window covering the visible frame. Right-click popups therefore only exist with the backdrop on.
- Focus: clicking any of our windows raises it in the model before hit-testing so OS and model z-orders agree; the key window follows OS focus. When no window of ours has focus (the app was deactivated), attached submenus close, as NeXT did.
- The miniaturize animation was dropped (there is no container to animate across); `--no-anim` was removed. `Hide` hides the whole application (`NSApplication.hide`), the macOS Dock icon brings it back. The `os_window` state field is kept for compatibility but unused.
- The drag image is an opaque 48×48 tile (softbuffer windows have no alpha on macOS) instead of a 50 % translucent icon.
- Headless mode composites all surfaces into one frame, so scripts and screenshots are unchanged.

- The Dock column is optional (`View ▸ Show Dock`, persisted as `dock_visible`) and **off by default**: an always-on-top column of tiles at the screen edge is intrusive on a shared desktop. The Recycler and application tiles stay. Default Dock contents are still resolved on first start so enabling it later shows the host apps.

- Miniwindow tiles and the Recycler ("black hole") tile are optional too (`View ▸ Show Miniwindows`, `View ▸ Show Recycler Tile`), **off by default**. With miniwindows hidden, miniaturizing just hides the window; `Windows ▸ File Viewer`, the new `Windows ▸ Recycler`, and `Tools ▸ Inspector…/Console…` bring windows back. With the tile hidden, files reach the Recycler via the Delete key and `File ▸ Empty Recycler` still works.

## Extra application: Shell (v0.3.2)

- A terminal window named as in 1.0. The emulation (PTY, VT parser, cell grid, scrollback) is `alacritty_terminal` 0.26; we only render cells with Liberation Mono 12 px in 7×14 cells, draw the cursor (block when key, hollow when not), forward keys as xterm byte sequences (arrows honour application-cursor mode, Ctrl-letter → control codes, Alt → ESC prefix) and map the scrollback onto our scroller. Terminal colors collapse to the four grays: chromatic text becomes dark gray so it stays legible on the white background; bold is faked by drawing the glyph twice.
- The shell starts in the home directory with `TERM=xterm-256color`; closing the window sends the event loop a shutdown (hang-up); when the child exits the title says so and the next key closes the window. Clipboard, mouse selection and mouse reporting are not implemented.
- On macOS, Cmd shortcuts still reach the menus while the Shell is key; on Windows the Shell takes all Ctrl combinations (so Ctrl-Q does not quit while it is key).

## Extra application: Mandelbrot (v0.3.1)

- Modelled on the 1.0 demo visible in the reference screenshot (white image panel, elapsed-time field, "Dithering" radio group with Standard PS / Knight's Tour / Ohlfs Mix / Error Diffusion, X/Y/Scale/Depth/Colors fields, one big button). There is no DSP, so it shows one image; the big button is Reset and a Save button writes `Mandelbrot-N.png` to the home folder.
- The four dithering modes are real algorithms in our own implementation: Bayer 4×4 ordered dither, an 8×8 threshold matrix generated from a knight's tour (Warnsdorff's rule), ordered dither mixed with hashed noise (an homage to Keith Ohlfs' pattern), and serpentine Floyd–Steinberg. Output is the four grays of the palette.
- Computation runs on a background thread; the window persists its position/open state like the others. Radio buttons are drawn as 13 px sunken circles, the one non-rectangular control in the app.

## NeXTSTEP 1.0 chrome (v0.2.1)

The owner asked for the look of the *first* NeXTSTEP release. Pixel geometry was measured from
1120×832 screenshots of NeXTSTEP 0.9/1.0 (toastytech.com, WinWorld); no artwork was copied, only
dimensions and shading rules. Where the PRD (which describes 3.3) and 1.0 differ, 1.0 wins:

- Window frame: 1 px black outline; no bevel around the content. Title bar = highlight row (light gray on the black key bar, white on the light non-key bar), 19 fill rows with the highlight down the left column and a dark right column, a dark row and a black row. **Non-key title bars are light gray with black text** (the PRD's dark-gray/white is the 3.x look). Buttons are 14×14 at 3 px; the miniaturize glyph is a tiny window (hollow square with a thick top), the close glyph a 2 px X.
- Resize bar: dark row, white row, 6 face rows; grooves (dark + white column) 28 px from each end instead of at 25 %/75 %. Panels (Inspector, Info, alerts) have no miniaturize button.
- Menus: as narrow as their contents (min 93 px), default position 0,0, left-aligned bold title on a raised black cell, items = white/light/dark/black rows (20 px), hollow ▷ for submenus, bare key letters (1.0 shows no modifier glyph), no separator before Hide. Attached submenus open with their **top aligned to the parent menu**, 2 px to its right.
- Browser: each column is a sunken box under a dark title cell showing the directory name, cells are 16 px with **white** selection highlight and hollow ▷ markers, no per-cell icons (neither 1.0 nor 3.3 had them), a ▼ ▲ button pair under every column instead of a knob scroller, and a ◀ ▶ strip on the left instead of a horizontal scroller.
- Scrollers (Console, Inspector, Recycler) sit on the **left** of their view, 18 px wide, with a 50 % dithered track, a dimpled knob and both arrows at the bottom.
- Dock/application/Recycler tiles use the heavier 1.0 bevel (2 px white, dark + 2 px black) and keep a 3 px margin from the screen edge; miniwindows carry a black title strip on top. The Recycler icon is a "black hole" (original drawing), as in 1.0.
- Alert panels: black title bar without text, icon + large "Alert" header, groove line, message, buttons bottom right (the default one with the return glyph only).

## Behaviour (unchanged from v0.1)

- `Entry.is_dir` for symlinks reflects the *target* (via `fs::metadata`, broken links fall back to the link itself). §8 says never follow, but §9.12 requires navigating into symlinked directories, which needs the target type.
- Copy-name clashes insert the suffix before the extension (`a copy.txt`), like the Finder, instead of literally appending.
- Recycler tile is pinned to the bottom-right corner (as on real NeXT screens) rather than glued under the last Dock tile, so the Dock strip between the last app tile and the Recycler is a real drop area for adding apps (§7.4).
- macOS app icons are extracted with the built-in `plutil` (icon file name from Info.plist) and `sips` (icns → PNG) instead of the `icns`/`png` crates for icns parsing. Windows uses PowerShell `System.Drawing.Icon.ExtractAssociatedIcon` (32 px, scaled) as the PRD's allowed fallback path.
- `~/.Trash` is protected by macOS TCC: listing it and `trash_is_empty` fail unless the app has Full Disk Access. The Recycler window explains this and offers "Open in Finder"; the tile then shows the empty state. Moving files *to* the trash (NSFileManager) works without it.
- Default scale threshold (§6.4 "1600×1000 device pixels") is evaluated in logical pixels: read literally, a Retina Mac at the default 1120×832 window would start at 2× with a 560×416 Screen that cannot hold the 640×480 File Viewer.
- Shelf icons are guessed from the path's extension (no stat) because the Shelf stores bare paths.
