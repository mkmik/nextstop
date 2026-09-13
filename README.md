# ReWorkspace

A single-binary recreation of the **look** of the NeXTSTEP 3.3 desktop — Workspace Manager,
vertical menus, Dock, Recycler — running as an ordinary window on Windows and macOS, in the
spirit of [ReProgman](https://github.com/mayuki/ReProgman).

It is not a picture: the File Viewer browses your real disk, double-click opens files with the
host OS, the Dock launches real applications, and dragging a file to the Recycler moves it to the
system trash.

![ReWorkspace at 1×](docs/screenshots/screen-1x.png)

At 2× (`View ▸ Scale ▸ 2×`):

![ReWorkspace at 2×](docs/screenshots/screen-2x.png)

> ReWorkspace is an original recreation of a *style*. It is not affiliated with, endorsed by, or
> derived from NeXT, Apple, or the GNUstep project. No artwork, fonts, or code from NeXTSTEP,
> OPENSTEP, macOS or GNUstep is included; every icon is an original drawing.

## Download & run

Builds are attached to [GitHub Releases](../../releases). Nothing is installed system-wide and
no administrator rights are needed; quit at any time.

**macOS (Apple Silicon)** — the app is not notarized. After copying `ReWorkspace.app` out of the
DMG, clear the quarantine flag once, exactly as ReProgman documents it:

```sh
xattr -cr /Applications/ReWorkspace.app
```

**Windows (x64, arm64)** — either run `ReWorkspace-portable-*.exe` directly or use the NSIS
installer (per-user, no admin). Windows 10/11 with the WebView2 runtime (built into Windows 11).

State lives in one file: `~/Library/Application Support/ReWorkspace/state.json` (macOS) or
`%APPDATA%\ReWorkspace\state.json` (Windows). Delete it to start fresh. A corrupt file is renamed
to `state.json.bad` and defaults are used.

## What works

- File Viewer: Shelf, Icon Path, multi-column Browser, keyboard navigation (arrows, Enter,
  Delete, type-ahead), hidden-file toggle, per-column scrollers, live refresh.
- Inspector: attributes, folder size on demand, text and image contents.
- Dock: default host apps (Terminal/TextEdit/Safari or Notepad/Calculator/Edge), launch on
  click, drag to reorder, drag off to remove, drop an app to add, drop a file on a tile to open
  it with that app.
- Recycler: drop to trash, contents window, `File ▸ Empty Recycler` (always confirms).
- Menus: full Workspace menu tree, submenus, tear-off menus that persist, key equivalents
  (Cmd on macOS, Ctrl on Windows), right-click main menu on the background.
- Persistence of window, menu, Dock, Shelf and scale settings.

## Known limitations

- **Windows Recycle Bin**: it is not a plain folder, so the Recycler window offers an
  *Open Recycle Bin* button instead of a listing and `Empty Recycler` is disabled.
- **macOS Trash listing** needs *Full Disk Access* (System Settings ▸ Privacy & Security). Without
  it, moving files to the Recycler still works, but the Recycler window cannot list `~/.Trash`
  and shows an *Open in Finder* button instead.
- Dock tiles always show the "not running" dots (except Workspace): host app state is not tracked.
- Windows app icons are extracted at 32 px and scaled.
- Browser view only (no Icon or Listing view), no Preferences, no search.

## Development

```sh
pnpm install
pnpm tauri dev            # dev build with hot reload
pnpm tauri build          # release bundle (.app/.dmg on macOS, .exe/NSIS on Windows)
cargo test --manifest-path src-tauri/Cargo.toml
pnpm exec tsc --noEmit && cargo clippy --manifest-path src-tauri/Cargo.toml
```

Look-review pages served by the dev server: `?demo=chrome` (bevels, fonts, icons),
`?demo=windows` (windows and scrollers), `?demo=menus` (menus, Info, Console; add `&alert`).
Opened in a plain browser they run against a small in-memory mock of the backend.

Pass `--no-anim` to disable the miniaturize animation.

See `docs/PRD.md` for the specification, `docs/DECISIONS.md` for deviations from it, and
`THIRD_PARTY_LICENSES.md` for bundled components (Liberation fonts, OFL 1.1).
