# Working on ReWorkspace

Design decisions and deviations from the original NeXTSTEP behaviour go in `docs/DECISIONS.md`;
anything a user can see goes in `README.md`.

## Every application gets a help page

`Info ▸ Help…` sits in every application menu and opens the page of whichever window is key, so a
new application window ships with its page in `PAGES` (`src/help.rs`) — otherwise it silently
falls back to the Workspace page.

A page is one or two sentences, then `key<tab>what it does` lines, and covers only what the window
does not already show; it is help, not the README. The panel has no scroller on purpose, and the
test in `src/help.rs` fails if a line is too wide for it, if a page is taller than the Screen, or
if a character has no glyph in the bundled fonts (`▸` has none — name menus in prose instead).
