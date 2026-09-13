import "./styles/palette.css";
import "./styles/chrome.css";
import { Screen, scale } from "./screen/screen";
import { el } from "./chrome/ui";
import { NXWindow } from "./chrome/window";
import { Scroller } from "./chrome/scroller";
import { Menu, MenuItem } from "./chrome/menu";
import { alert } from "./chrome/alert";
import { createConsole, createInfo, log } from "./apps/workspace/panels";
import { call, hasTauri } from "./backend";
import { icon } from "./icons";
import { State, defaults, dirty } from "./state";

const screen = new Screen();
const demo = new URLSearchParams(location.search).get("demo");
const state: State = defaults("/");

// ---- lazily created panels
let consoleWin: NXWindow | null = null, infoWin: NXWindow | null = null;
const showConsole = () => {
  const c = state.windows.console;
  (consoleWin ??= createConsole(screen, { x: c.x, y: c.y, w: c.w, h: c.h,
    onMove() { c.x = consoleWin!.x; c.y = consoleWin!.y; dirty(); },
    onResize() { c.w = consoleWin!.w; c.h = consoleWin!.h; dirty(); },
    onClose() { c.open = false; dirty(); } })).show();
  c.open = true; dirty();
};
const showInfo = () => (infoWin ??= createInfo(screen)).show();
const nyi = (name: string) => () => log(`not implemented: ${name}`);

async function hide() {
  if (!hasTauri) return log("Hide: no OS window in the browser");
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().minimize();
}
async function quit() { await saveNow(); call("quit").catch(() => log("Quit: no backend")); }
let saveNow = async () => {}; // replaced in M5

// ---- main menu (§7.3, exhaustive v1 contents)
const mainItems: MenuItem[] = [
  { label: "Info", submenu: [
    { label: "Info Panel…", action: showInfo },
    { label: "Preferences…", disabled: true },
    { label: "Help…", disabled: true } ] },
  { label: "File", submenu: [
    { label: "Open", key: "o", action: nyi("Open") },
    { label: "Open as Folder", key: "O", disabled: true },
    { label: "New Folder", key: "n", action: nyi("New Folder") },
    { label: "Duplicate", key: "d", action: nyi("Duplicate") },
    { label: "Compress", disabled: true },
    { label: "Destroy", key: "r", action: nyi("Destroy") },
    { label: "Empty Recycler", action: nyi("Empty Recycler") } ] },
  { label: "Edit", submenu: [
    { label: "Cut", key: "x", disabled: true },
    { label: "Copy", key: "c", action: nyi("Copy") },
    { label: "Paste", key: "v", action: nyi("Paste") },
    { label: "Select All", key: "a", action: nyi("Select All") } ] },
  { label: "Disk", submenu: [
    { label: "Check for Disks", action: nyi("Check for Disks") },
    { label: "Eject", disabled: true } ] },
  { label: "View", submenu: [
    { label: "Browser", checked: () => true, action: () => {} },
    { label: "Icon", disabled: true },
    { label: "Listing", disabled: true },
    { label: "Scale", submenu: [
      { label: "1×", checked: () => scale === 1, action: () => setScale(1) },
      { label: "2×", checked: () => scale === 2, action: () => setScale(2) } ] },
    { label: "Show Hidden Files", checked: () => state.show_hidden, action: () => { state.show_hidden = !state.show_hidden; dirty(); hooks.hiddenChanged(); } } ] },
  { label: "Tools", submenu: [
    { label: "Inspector…", key: "i", action: nyi("Inspector") },
    { label: "Finder…", disabled: true },
    { label: "Processes…", disabled: true },
    { label: "Console…", action: showConsole } ] },
  { label: "Windows", submenu: [
    { label: "File Viewer", action: nyi("File Viewer") },
    { label: "Arrange in Front", action: () => screen.wins.filter((w) => w.visible).sort((a, b) => +a.el.style.zIndex - +b.el.style.zIndex).forEach((w) => screen.front(w)) },
    { label: "Miniaturize Window", key: "m", action: () => (screen.keyWin as NXWindow | null)?.miniaturize() },
    { label: "Close Window", key: "w", action: () => (screen.keyWin as NXWindow | null)?.close() } ] },
  { label: "Services", submenu: [{ label: "No Services Available", disabled: true }] },
  { label: "Hide", key: "h", gapBefore: true, action: hide },
  { label: "Quit", key: "q", action: quit },
];

/** Hooks filled in by later milestones (File Viewer, Dock…). */
const hooks = { hiddenChanged: () => {} };

function setScale(s: 1 | 2) { state.scale = s; screen.setScale(s); dirty(); }

const menuOpts = {
  onMove: (m: Menu) => {
    if (m.opts.main) state.menu_pos = { x: m.x, y: m.y };
    else { const t = state.torn_menus.find((t) => t.path.join("/") === m.key); if (t) { t.x = m.x; t.y = m.y; } }
    dirty();
  },
  onTear: (m: Menu) => { if (!state.torn_menus.some((t) => t.path.join("/") === m.key)) state.torn_menus.push({ path: m.opts.path!, x: m.x, y: m.y }); dirty(); },
  onCloseTorn: (m: Menu) => { state.torn_menus = state.torn_menus.filter((t) => t.path.join("/") !== m.key); dirty(); },
};

function buildMenus() {
  new Menu(screen, "Workspace", mainItems, { x: state.menu_pos.x, y: state.menu_pos.y, main: true, ...menuOpts });
  for (const t of state.torn_menus) Menu.openTorn(screen, mainItems, t.path, t.x, t.y, menuOpts);
  // §9.9: right-click on the Screen background pops a copy of the main menu under the cursor.
  screen.el.addEventListener("mousedown", (e) => {
    if (e.button !== 2 || e.target !== screen.el) return;
    Menu.closeAll();
    new Menu(screen, "Workspace", mainItems, { x: e.clientX / scale, y: e.clientY / scale, main: true, transient: true, ...menuOpts });
  });
  document.addEventListener("keydown", (e) => {
    if (Menu.handleKey(mainItems, e)) { e.preventDefault(); return; }
    if ((screen.keyWin as NXWindow | null)?.opts.onKeyDown?.(e)) e.preventDefault();
  });
}

// ---- boot
if (demo === "chrome") demoChrome();
else if (demo === "windows") demoWindows();
else if (demo === "menus") demoMenus();
else buildMenus();

function demoChrome() {
  const box = el("div", "demo");
  const icons = el("div", "demo-row nx-raised demo-icons");
  for (const name of Object.keys(icon).sort()) { const i = el("img"); i.src = icon[name]; i.title = name; icons.append(i); }
  box.append(
    el("div", "demo-row", el("div", "nx-raised nx-button", "Raised Button"), el("div", "nx-sunken nx-well", "Sunken well")),
    el("div", "demo-row", el("span", "", "Regular 12 px"), el("span", "nx-bold", "Bold 12 px"), el("span", "nx-small", "Small 10 px"), el("span", "nx-mono", "Mono 11 px")),
    icons,
  );
  screen.el.append(box);
}

function demoWindows() {
  const a = new NXWindow(screen, { title: "Untitled", x: 60, y: 60, w: 320, h: 240, minW: 160, minH: 100 });
  const b = new NXWindow(screen, { title: "Scroller Demo", x: 300, y: 200, w: 300, h: 260, minW: 160, minH: 100 });
  const list = el("div", "demo-list");
  for (let i = 1; i <= 100; i++) list.append(el("div", "demo-line", `Line ${i}`));
  const view = el("div", "demo-view", list);
  const sc = new Scroller(view, true);
  const row = el("div", "demo-scrollrow", view, sc.el);
  const hview = el("div", "demo-hview", el("div", "demo-hcontent", "This line is much wider than the window so the horizontal scroller gets a knob. ".repeat(3)));
  const hsc = new Scroller(hview, false);
  b.content.append(row, hview, hsc.el);
  b.content.style.display = "flex"; b.content.style.flexDirection = "column";
  a.show(); b.show();
}

function demoMenus() {
  buildMenus();
  log("demo=menus started");
  showConsole();
  showInfo();
  if (new URLSearchParams(location.search).has("alert"))
    alert(screen, { message: "Are you sure you want to empty the Recycler?", detail: "This cannot be undone.", buttons: ["Cancel", "Empty"] }).then((i) => log(`alert → ${i}`));
}
