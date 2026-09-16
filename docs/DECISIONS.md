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

## NeXTSTEP 2.0 look (v0.4)

The owner then asked for the 2.0 (1990) aesthetics. Measured from toastytech's 2.0 screenshots; where 2.0 differs from 1.0:

- **File Viewer** is the 2.0 layout the PRD describes: shelf icons with 12 px labels straight on the window face and a "…MB available on hard disk" line (`statvfs`; absent on Windows); a sunken well holding the **icon path** with one icon centred above each browser column (root shown as the computer icon, without label), hollow ▷ markers between them, a white box behind the leaf icon and its label, and the **horizontal column scroller** inside the same frame (knob and dimple, no arrows); the browser is a sunken box in which every column is an 18 px **scroller strip** (light margin, 16 px slot with knob and ▲▼ when the column overflows, light margin) plus a 121 px list, separated by a black line; the box has no bottom edge and runs to the resize bar. Column title cells and the ◀▶ strip of 1.0 are gone. Cells are 15 px.
- **Menus**: a pressed or open item is highlighted **white** with black text (1.0 inverted it to black). The only drop shadow is the 1 px black column on the right; the menu ends with the last item's black row, and every cell's white top row and left column run through the corners (dark edges start one pixel in). Outside that white highlight the menu carries a 1 px **dark-gray bezel** along its top and left, so the whole frame reads dark → white → face → dark → black. It is invisible in most 1.0/2.0 screenshots because menus sit at the Screen corner and it falls off-screen, but `ns20services.png` shows it at x=176: the column between a parent's black shadow and an attached submenu stays dark gray over a window that is black and white behind it, which is also why submenus are offset by 2 px (shadow + bezel). Each menu's window is therefore 1 px wider and taller than its face, up and to the left. Everything else (22 px title bars, glyphs, resize grooves, left-hand scrollers) is unchanged from 1.0.
- ▷ markers are the 2.0 glyph measured from the screenshots: a 7×7 engraved triangle (1 px black bar, dark-gray upper edge, white lower edge — light gray on a white selected cell — open face), the same in menus, browser cells and the icon path, with its bar 12 px left of the cell edge. Dock/application/miniwindow tiles use the thinner 2.0 bevel (2 px white, 1 px dark + 1 px black). The Recycler is the 2.0 recycling symbol again (an original three-arrow drawing) instead of the 1.0 black hole.

## Fractional display scale (v0.4.2)

- The scale is a real number (state `scale` is now a float; old integer values load fine). `--scale <factor>` overrides it for one run without saving; `View ▸ Scale` gained 1.5×. Everything is drawn through the painter's logical→device mapping, fonts via fontdue and icons via resvg at the target size, so non-integer factors are genuinely re-rendered; only the 1 px bevel lines alternate between 1 and 2 device pixels, and the 50 % dither loses its regularity, which is inherent to any non-integer factor. NeXTSTEP itself, despite Display PostScript's device independence, offered no such setting: its chrome and icons were TIFF bitmaps laid out for one point per pixel.

## Icons in the 2.0 idiom (v0.4.1)

- All 48 px icons were redrawn in the style of the 2.0 stock icons: grayscale only (no accent colours), objects in shallow 3D with a lit top face and a dark side, black outlines, white highlights, 50 % dither for shading and roofs, and a dithered drop shadow to the lower right. Documents share one page shape with a folded corner. They remain original drawings; nothing was traced from NeXT bitmaps (§12).
- The 16 px cell icons were removed with the 1.0 browser; the Browser shows names only, as NeXTSTEP did.

## Help panels (v0.5)

- NeXTSTEP applications each had their own help, served out of Digital Librarian-indexed RTF. Ours is one panel (`src/help.rs`) with a page per application, the page of the window whose menu `Info ▸ Help…` came from — the same Info menu is in every application menu, so the command is everywhere without a help window per application. The panel sizes itself to its page and has no scroller, so pages are short by construction and a test enforces that they fit. Pages are plain text with a key column, not RTF, and there is no index, search or cross-reference.
- The bundled Liberation faces have no `▸`, so page text avoids it and names menus in prose (“New Row, in the Item menu”); the test checks every character has a glyph.

## A menu for every application (v0.5.1)

- Improv and Concurrence had menus of their own; the Shell, the Librarian and Mandelbrot borrowed Workspace's, which offered them File, Disk, View and Tools and nothing of their own. `APP_MENUS` (`src/chrome.rs`) now maps the key window to its menu and `resolve_path` walks the same table, so an application declares its menu in one place and its torn-off submenus come back with it. The shape is `Info`, the application's own commands, `Windows`, `Hide`, `Quit` — no `Tools`, because only Workspace launches things.
- The commands are the ones the window already offers as buttons, through the same call (`Dither ▸ Knight's Tour` *is* the radio button, `Image ▸ Reset` *is* the Reset button), so there is one implementation per command and `item_state` reads the same fields the buttons draw from: the dithering in force is checked, `Open Document` is disabled until a hit is selected, `Save` until there is an image.
- The Shell had no buttons, so it gets the two commands a terminal needs. `New Shell` (Cmd-N) restarts the login shell in the window — also the way back after one has exited; NeXT's Terminal opened a new window for it, and ours has one Shell window. `Clear Buffer` (Cmd-K) clears the screen and the scrollback and puts the cursor home. It deliberately tells the shell nothing: a Ctrl-L would have it redraw its prompt, but the redraw scrolls a line straight back into the scrollback just emptied, so the window stays blank until the next Return.
- Panels do not take the menu from their application: `make_key` remembers the last key window that was not one (`menu_key`, read through `App::menu_owner`), so an application's Info or Help panel leaves the menu — and the Help page — its own. The Info panel used to hand the menu back to Workspace.
- A torn-off menu's path now starts with the application it came from: `Concurrence ▸ View` and `Workspace ▸ View` are different menus that shared a path, and therefore one entry in `state.json` and one torn window. Paths written before this are bare labels and are still resolved the old way, by scanning every root.

## Preferences (v0.5.2)

- Modelled on the 1.0 application in toastytech's `ns10prefs.png`: one 392×319 window whose top is a
  sunken scroll view of 69×65 module cells (2 px dark/black separator, 1 px white highlight, a 66 px
  face, 48 px icon, white when selected) with the horizontal scroller under them, a groove, and then
  the pane of the selected module — its name in **oblique** text at the top left, as in *Date & Time
  Preferences*, over a bordered group whose title breaks its top edge. Every measurement in that list
  is from the screenshot; the switch and its check mark (white stroke, black shadow, 7×7 in a 15 px
  box white on top and left, black on the other two) are measured from `ns09prefs.png`.
- **The modules are the settings this program has**, not the original's: Display (the scale),
  Workspace (Dock, miniwindow and Recycler tiles, Screen Backdrop) and Expert (hidden files, and
  where `state.json` is). Date & Time, Sound, Password, Localization, Font and UNIX Expert set
  things we do not own, so they are not there — with three modules the icon row never scrolls, and
  the strip under it is drawn as NeXTSTEP drew the scroller of a view that does not scroll: the
  frame and the empty dithered slot, no knob and no arrows.
- **A switch is the View menu command of the same name, through the same call**, so `item_state` is
  what draws both and they cannot disagree (`Act::ShowDock` and friends, as AGENTS.md prescribes).
- **The scale is a slider** — the one control the original used for a continuous quantity — between
  0.5× and 4× in quarter steps, and it is applied when the knob is let go, not while it is dragged:
  every new factor resizes the window under the pointer, so a live knob runs away from the mouse.
  It is `View ▸ Scale`'s setting and `--scale`'s, kept in `state.json` like the menu's.
- Oblique text is faked in the painter (`text_oblique`): the bundled Liberation faces have no
  italic, so each glyph row is sheared right by 0.21 px per pixel above the baseline, which is what
  Helvetica Oblique's lean measures on the screenshot. Widths are the upright ones.
- Deviations: the window has our close and miniaturize buttons (the original had neither — it was a
  separate process you quit, and this one is a window in ours), its menu is the original's without
  `Edit` (nothing here is typed into), and `Info ▸ Preferences…` opens it from *every* application's
  menu, because this program has one set of preferences rather than one per application.

## Extra application: Concurrence (v0.5)

- Lighthouse Design's Concurrence was an outliner, a slide editor and a presenter over one document. Ours keeps the outliner and the presenter and reduces the editor to the one layout the outline already implies: a level-0 topic is a slide title, its descendants are its bullets, indented and drawn smaller with a square or dash marker. No styles, masters, images, charts, transitions or speaker notes — the slide is a rendering of the outline, not a second document.
- Its menu is the one from Lighthouse's own 1995 data sheet (Info, Document, Edit, Format, Arrange, Topics/Slides, Views, Tools, Windows, Print, Services, Hide, Quit) cut down to the commands this clone has. `Topics ▸ Move ▸ Move Left ([) / Move Right (])` is kept verbatim, key equivalents included, because the manual (ch. 26, appendix B) makes Tab and Shift-Tab shortcuts *for those menu commands* rather than the only way to indent. Move Up/Down, New Topic, Speaker Notes, Clone, Follow Master and Start New Page are not implemented, so they are not in the menu. `View` and `Save` are ours — the original reached its outline/slide/show views through a View Browser over several windows, which this one-window clone does not have.
- The outline is a flat `Vec<Row { text, level, collapsed }>`; a subtree is just the following rows with a greater level, which turns demote/promote, collapse and "the bullets of this slide" into range operations. It is persisted in `state.json` (one tab-indented line per topic) like every other piece of state, and `Save` exports the same text as `Presentation-N.txt` in the home folder, as Mandelbrot's Save does for PNGs. There is no Open and no document window: this application has one document.
- The show is the same window with `Win::chrome = false` and the rect of the whole Screen, and `surfaces()` drops the menus, Dock and tiles while it runs, so a presentation is really full-screen instead of a window with a title bar under a floating menu. Escape, a click past the last slide, or closing the window restores the saved rect and the chrome.
- Text editing is a caret in one row (insert, Backspace/Delete, Left/Right/Home/End, click to place it); there is no selection, no undo, and no wrapping.

## Extra application: Shell (v0.3.2)

- A terminal window named as in 1.0. The emulation (PTY, VT parser, cell grid, scrollback) is `alacritty_terminal` 0.26; we only render cells with Liberation Mono 12 px in 7×14 cells, draw the cursor (block when key, hollow when not), forward keys as xterm byte sequences (arrows honour application-cursor mode, Ctrl-letter → control codes, Alt → ESC prefix) and map the scrollback onto our scroller. Terminal colors collapse to the four grays: chromatic text becomes dark gray so it stays legible on the white background; bold is faked by drawing the glyph twice.
- The shell starts in the home directory with `TERM=xterm-256color`; closing the window sends the event loop a shutdown (hang-up); when the child exits the title says so and the next key closes the window. Clipboard, mouse selection and mouse reporting are not implemented.
- On macOS, Cmd shortcuts still reach the menus while the Shell is key; on Windows the Shell takes all Ctrl combinations (so Ctrl-Q does not quit while it is key).

## Extra application: Digital Librarian

- Modelled on the 1990 NeXT application: a row of bookshelves above a "Find:" field and a ranked
  list of documents. **The File Viewer Shelf is the bookshelf** — one persisted list of folders,
  and dragging a folder onto the Shelf is exactly the gesture that added a bookshelf on a real
  NeXT. Clicking a bookshelf selects it (click again to deselect); opening the window selects the
  home folder, so Search can never mean "the whole disk" by accident.
- **No index.** The original shipped `ixbuild` because the hardware was a 25 MHz 68030; we grep on
  a background thread instead and cap a search at 4000 documents, 300 hits and 1 MB per document.
  Documents are the text and source types of the icon map (`icons::is_text_like`); dot files and
  dot folders are skipped, and `DirEntry::file_type` is used so symlinks are never followed into a
  loop. Ranking is hit count, then name — no relevance weighting.
- Results are a 16 px row list in the browser idiom (white selection, no per-row icons) showing the
  name, the first matching line and the hit count. Double-click opens the document with the host OS;
  Return in the field searches, the arrow keys walk the results.

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
- Scrollers (Console, Inspector, Recycler, Shell) sit on the **left** of their view, 21 px wide, measured from the 2.0 shots (the 1.0 Librarian scroller is the same): dark+black frame on top/left, a 1 px light margin, a 16 px dithered slot holding the 15 px knob and arrow faces plus their black shadow, another light margin and a black edge on the right; the round 6×6 dimple; both arrows stacked at the bottom with a light gap row and a light margin row below. The arrow is the 9-row 2.0 triangle whose step corners are dark gray. A scroller whose content fits draws only the slot — no knob, no arrows — exactly as NeXTSTEP's disabled scroller did.
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
