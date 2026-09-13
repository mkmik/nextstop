import "./styles/palette.css";
import "./styles/chrome.css";
import { Screen, scale } from "./screen/screen";
import { el, isMac } from "./chrome/ui";
import { NXWindow, setAnimations } from "./chrome/window";
import { Scroller } from "./chrome/scroller";
import { Menu, MenuItem } from "./chrome/menu";
import { alert } from "./chrome/alert";
import { createConsole, createInfo, log } from "./apps/workspace/panels";
import { call, hasTauri } from "./backend";
import { icon } from "./icons";
import { State, defaults, dirty, onDirty } from "./state";
import { FileViewer } from "./apps/workspace/fileviewer";
import { Inspector } from "./apps/workspace/inspector";
import { Dock } from "./dock/dock";
import { setWindowsPaths, join } from "./paths";

const screen = new Screen();
const demo = new URLSearchParams(location.search).get("demo");
const state: State = defaults("/");
let fv: FileViewer;
let inspector: Inspector | null = null;
let dock: Dock;
let clipboard: string[] = [];
const showInspector = () => {
  const g = state.windows.inspector;
  (inspector ??= new Inspector(screen, state)).win.show();
  inspector.update(fv.selEntry);
  g.open = true; dirty();
};
const showFileViewer = () => { fv.win.show(); state.windows.file_viewer.open = true; dirty(); };

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

async function hide() {
  if (!hasTauri) return log("Hide: no OS window in the browser");
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().minimize();
}
async function quit() { await saveNow(); call("quit").catch(() => log("Quit: no backend")); }
let saveNow: () => Promise<void> = async () => {};
interface Loaded { state: Partial<State> | null; no_anim: boolean; message: string | null }

// ---- main menu (§7.3, exhaustive v1 contents)
const mainItems: MenuItem[] = [
  { label: "Info", submenu: [
    { label: "Info Panel…", action: showInfo },
    { label: "Preferences…", disabled: true },
    { label: "Help…", disabled: true } ] },
  { label: "File", submenu: [
    { label: "Open", key: "o", action: () => fv.open() },
    { label: "Open as Folder", key: "O", disabled: true },
    { label: "New Folder", key: "n", action: () => fv.newFolder() },
    { label: "Duplicate", key: "d", action: () => fv.duplicate() },
    { label: "Compress", disabled: true },
    { label: "Destroy", key: "r", action: () => fv.destroySelection() },
    { label: "Empty Recycler", disabled: () => !isMac, action: () => dock.emptyRecycler() } ] },
  { label: "Edit", submenu: [
    { label: "Cut", key: "x", disabled: true },
    { label: "Copy", key: "c", action: () => { clipboard = fv.deepSelection.map((e) => e.path); log(`copied ${clipboard.length} path(s)`); } },
    { label: "Paste", key: "v", disabled: () => clipboard.length === 0, action: () => fv.paste(clipboard) },
    { label: "Select All", key: "a", action: () => fv.selectAll() } ] },
  { label: "Disk", submenu: [
    { label: "Check for Disks", action: () => fv.refresh() },
    { label: "Eject", disabled: true } ] },
  { label: "View", submenu: [
    { label: "Browser", checked: () => true, action: () => {} },
    { label: "Icon", disabled: true },
    { label: "Listing", disabled: true },
    { label: "Scale", submenu: [
      { label: "1×", checked: () => scale === 1, action: () => setScale(1) },
      { label: "2×", checked: () => scale === 2, action: () => setScale(2) } ] },
    { label: "Show Hidden Files", checked: () => state.show_hidden, action: () => { state.show_hidden = !state.show_hidden; dirty(); fv.refresh(); } } ] },
  { label: "Tools", submenu: [
    { label: "Inspector…", key: "i", action: showInspector },
    { label: "Finder…", disabled: true },
    { label: "Processes…", disabled: true },
    { label: "Console…", action: showConsole } ] },
  { label: "Windows", submenu: [
    { label: "File Viewer", action: showFileViewer },
    { label: "Arrange in Front", action: () => screen.wins.filter((w) => w.visible).sort((a, b) => +a.el.style.zIndex - +b.el.style.zIndex).forEach((w) => screen.front(w)) },
    { label: "Miniaturize Window", key: "m", action: () => (screen.keyWin as NXWindow | null)?.miniaturize() },
    { label: "Close Window", key: "w", action: () => (screen.keyWin as NXWindow | null)?.close() } ] },
  { label: "Services", submenu: [{ label: "No Services Available", disabled: true }] },
  { label: "Hide", key: "h", gapBefore: true, action: hide },
  { label: "Quit", key: "q", action: quit },
];

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

// ---- boot (§10): chrome first, directory listings arrive later
async function boot() {
  const [home, roots, defaultDock, loaded] = await Promise.all([
    call<string>("home_dir"), call<string[]>("root_dirs"), call<string[]>("default_dock"), call<Loaded>("load_state")]);
  setWindowsPaths(roots[0] !== "/");
  const base = defaults(home);
  base.dock = defaultDock;
  base.shelf = [home, roots[0], roots[0] === "/" ? "/Applications" : "C:\\Program Files", join(home, "Desktop"), join(home, "Documents")];
  base.scale = innerWidth >= 1600 && innerHeight >= 1000 ? 2 : 1; // §6.4 (threshold read in CSS px, see DECISIONS)
  const saved = loaded.state ?? {};
  Object.assign(state, base, saved, { version: 1, windows: { ...base.windows, ...(saved.windows ?? {}) } });
  if (state.scale !== 2) state.scale = 1;
  if (loaded.message) log(loaded.message);
  // §11: paths that no longer exist are dropped and logged
  const keep = async (paths: string[], what: string) => {
    const ok = await call<string[]>("filter_existing", { paths });
    paths.filter((p) => !ok.includes(p)).forEach((p) => log(`dropped missing ${what} entry ${p}`));
    return ok;
  };
  [state.dock, state.shelf] = await Promise.all([keep(state.dock, "Dock"), keep(state.shelf, "Shelf")]);
  if (!(await keep([state.windows.file_viewer.path], "File Viewer")).length) state.windows.file_viewer.path = home;
  setAnimations(state.animations && !loaded.no_anim);
  screen.setScale(state.scale);
  fv = new FileViewer(screen, state, home, roots);
  fv.onSelection = (sel) => inspector?.update(sel[0] ?? null);
  fv.renderShelf();
  dock = new Dock(screen, state, home, { showFileViewer, fsChanged: () => fv.refresh() });
  fv.onFsChange = () => dock.updateTrash();
  buildMenus();
  if (state.windows.file_viewer.open) fv.win.show();
  if (state.windows.inspector.open) showInspector();
  if (state.windows.console.open) showConsole();
  // persistence: debounced 2 s after any change, on OS window close, and on Quit
  saveNow = () => call("save_state", { state }).then(() => {}, (e) => log(`save failed: ${e}`));
  let timer = 0;
  onDirty(() => { clearTimeout(timer); timer = window.setTimeout(saveNow, 2000); });
  window.addEventListener("resize", () => { state.os_window = { w: innerWidth, h: innerHeight }; dirty(); });
  if (hasTauri) {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const w = getCurrentWindow();
    w.onCloseRequested(async (e) => { e.preventDefault(); await saveNow(); await w.destroy(); });
  }
  log("ReWorkspace started");
  await fv.navigate(state.windows.file_viewer.path);
  setInterval(() => { if (document.hasFocus()) fv.refresh(); }, 5000);
  window.addEventListener("focus", () => fv.refresh());
}

if (demo === "chrome") demoChrome();
else if (demo === "windows") demoWindows();
else boot().then(() => { if (demo === "menus") demoMenus(); }, (e) => { log(`boot failed: ${e}`); buildMenus(); showConsole(); });

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
  log("demo=menus started");
  showConsole();
  showInfo();
  if (new URLSearchParams(location.search).has("alert"))
    alert(screen, { message: "Are you sure you want to empty the Recycler?", detail: "This cannot be undone.", buttons: ["Cancel", "Empty"] }).then((i) => log(`alert → ${i}`));
}
