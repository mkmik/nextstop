# Decisions / deviations from the PRD

One line per deviation, with the reason.

## Native rewrite (v0.2)

- **Stack change requested by the owner**: the Tauri + HTML/TS frontend (kept in git history as tag `v0.1.0-tauri`) was replaced by a native Rust program. The PRD ruled out egui/iced because bevels and text metrics are painful there; instead the UI is drawn by our own software painter (`src/paint.rs`) into a `softbuffer` framebuffer in a `winit` window. Text is rasterized from the bundled Liberation TTFs with `fontdue`, icons from our SVGs with `resvg`. No webview, no JavaScript, no GPU.
- The PRD's repository layout, `?demo=` pages and Playwright notes are web-specific. Equivalents: `--demo chrome` renders the bevel/font/icon test screen; `--headless <script> --out <dir>` replays mouse/keyboard events and writes PNG screenshots (this is how every screenshot in `docs/screenshots` was made and how the app is tested without a display).
- `--home <dir>` and `--config <dir>` override the home and config directories so tests run against a throwaway tree.
- Milestone screenshots `m0…m6` belonged to the web version and were removed; the native screenshots are named by content.
- macOS packaging is `scripts/bundle-macos.sh` (plain `.app` + `hdiutil` DMG, ad-hoc codesign); Windows ships the bare `reworkspace.exe`, which is fully portable (no WebView2 needed any more). The optional NSIS installer and an embedded `.exe` icon were not done.
- Miniaturize animation shrinks a gray window outline (120 ms) instead of scaling the live window contents.

## Behaviour (unchanged from v0.1)

- `Entry.is_dir` for symlinks reflects the *target* (via `fs::metadata`, broken links fall back to the link itself). §8 says never follow, but §9.12 requires navigating into symlinked directories, which needs the target type.
- Copy-name clashes insert the suffix before the extension (`a copy.txt`), like the Finder, instead of literally appending.
- Recycler tile is pinned to the bottom-right corner (as on real NeXT screens) rather than glued under the last Dock tile, so the Dock strip between the last app tile and the Recycler is a real drop area for adding apps (§7.4).
- macOS app icons are extracted with the built-in `plutil` (icon file name from Info.plist) and `sips` (icns → PNG) instead of the `icns`/`png` crates for icns parsing. Windows uses PowerShell `System.Drawing.Icon.ExtractAssociatedIcon` (32 px, scaled) as the PRD's allowed fallback path.
- `~/.Trash` is protected by macOS TCC: listing it and `trash_is_empty` fail unless the app has Full Disk Access. The Recycler window explains this and offers "Open in Finder"; the tile then shows the empty state. Moving files *to* the trash (NSFileManager) works without it.
- Default scale threshold (§6.4 "1600×1000 device pixels") is evaluated in logical pixels: read literally, a Retina Mac at the default 1120×832 window would start at 2× with a 560×416 Screen that cannot hold the 640×480 File Viewer.
- Shelf icons are guessed from the path's extension (no stat) because the Shelf stores bare paths.
