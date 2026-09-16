# Working on NeXTSTOP

Design decisions and deviations from the original NeXTSTEP behaviour go in `docs/DECISIONS.md`;
anything a user can see goes in `README.md`.

## Every application gets a menu

A NeXTSTEP application owns the main menu while its window is key, so a new application window
ships with its menu in `APP_MENUS` (`src/chrome.rs`) — otherwise it borrows Workspace's, which
offers it File, Disk, View and Tools and nothing of its own. The shape is `Info`, the application's
own commands, `Windows`, `Hide`, `Quit`; there is no `Tools`, because only Workspace launches
things. A command that is already a button in the window goes through the same call, so there is
one implementation and `item_state` can check or disable the item from the same state.

Panels (`Info`, `Help`, alerts) belong to the application that opened them and do not take the
menu: `App::menu_owner` is the window the menu and the Help page follow.

## Every application gets a help page

`Info ▸ Help…` sits in every application menu and opens the page of the window whose menu it came
from, so a new application window ships with its page in `PAGES` (`src/help.rs`) — otherwise it
silently falls back to the Workspace page.

A page is one or two sentences, then `key<tab>what it does` lines, and covers only what the window
does not already show; it is help, not the README. The panel has no scroller on purpose, and the
test in `src/help.rs` fails if a line is too wide for it, if a page is taller than the Screen, or
if a character has no glyph in the bundled fonts (`▸` has none — name menus in prose instead).
