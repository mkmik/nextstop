# Decisions / deviations from the PRD

One line per deviation, with the reason.

- Screenshots: `screencapture -l` of the real app is blocked on the dev machine (no Screen Recording permission for the shell), so milestone screenshots are rendered headlessly from the Vite dev server with Playwright/Chromium at 1×. Same DOM/CSS, same fonts; only the OS window frame is missing.
- `Entry.is_dir` for symlinks reflects the *target* (via `fs::metadata`, broken links fall back to the link itself). §8 says never follow, but §9.12 requires navigating into symlinked directories, which needs the target type. Local stat is cheap.
- Extra backend command `read_file_b64(path, max_bytes)` for the Inspector's image viewer (the PRD lists only `read_text_head`); same path checks apply. Also `filter_existing(paths)` to drop vanished state paths, `platform()` for the Info panel, `quit()`.
- Copy-name clashes insert the suffix before the extension (`a copy.txt`), like the Finder, instead of literally appending.
- Recycler tile is pinned to the bottom-right corner (as on real NeXT screens) rather than glued under the last Dock tile, so the Dock strip between the last app tile and the Recycler is a real drop area for adding apps (§7.4).
- Headless review renders (Playwright/Chromium) use a small in-browser mock of the backend (`src/mock.ts`); it is only reached when `window.__TAURI_INTERNALS__` is absent.
