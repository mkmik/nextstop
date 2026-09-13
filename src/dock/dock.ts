import { el, svg, isMac } from "../chrome/ui";
import { Screen, pressable, trackDrag } from "../screen/screen";
import { NXWindow } from "../chrome/window";
import { Scroller } from "../chrome/scroller";
import { alert } from "../chrome/alert";
import { call, Entry } from "../backend";
import { icon, iconFor, ext } from "../icons";
import { State, dirty } from "../state";
import { basename, join } from "../paths";
import { dropTarget } from "../screen/dnd";
import { log } from "../apps/workspace/panels";

const TILE = 64;
const isAppPath = (p: string) => ["app", "exe", "lnk"].includes(ext(basename(p)));

const iconCache = new Map<string, Promise<string>>();
/** Host app icon as a data URL (Rust extracts and caches the PNG); falls back to our generic icon. */
export function appIcon(app: string): Promise<string> {
  let p = iconCache.get(app);
  if (!p) {
    p = call<string>("app_icon_png", { app }).then((b64) => `data:image/png;base64,${b64}`, (e) => { log(`icon for ${basename(app)}: ${e}`); return icon["application"]; });
    iconCache.set(app, p);
  }
  return p;
}

function tile(iconUrl: string, dots: boolean) {
  const img = el("img"); img.src = iconUrl;
  const t = el("div", "nx-raised nx-tile", img);
  if (dots) t.append(svg(12, 2, `<rect width="2" height="2"/><rect x="4" width="2" height="2"/><rect x="8" width="2" height="2"/>`, "dots"));
  return t;
}

/** Dock (§7.5), application tile (§7.6) and Recycler (§7.7). */
export class Dock {
  el = el("div", "dock");
  recycler = el("div", "nx-raised nx-tile recycler");
  private recImg = el("img");
  private recWin: NXWindow | null = null;
  private recBody = el("div", "rec-grid");
  private recScroller: Scroller | null = null;
  private warned = false;
  trashEmpty = true;

  constructor(public screen: Screen, public state: State, public home: string, public hooks: { showFileViewer: () => void; fsChanged: () => void }) {
    const logo = el("img"); logo.src = icon["workspace"];
    screen.tiles.append(this.el, this.recycler, el("div", "nx-raised nx-tile apptile", logo));
    this.recycler.append(this.recImg);
    pressable(this.recycler, () => this.openRecycler());
    dropTarget(this.recycler, (paths) => this.trash(paths));
    dropTarget(this.el, (paths) => paths.filter(isAppPath).forEach((p) => this.add(p)));
    this.render();
    this.updateTrash();
  }

  render() {
    const ws = tile(icon["workspace"], false);
    ws.title = "Workspace";
    pressable(ws, this.hooks.showFileViewer);
    this.el.replaceChildren(ws, ...this.state.dock.slice(0, 12).map((app, i) => this.appTile(app, i)));
  }

  private appTile(app: string, i: number) {
    const t = tile(icon["application"], true);
    const img = t.querySelector("img")!;
    appIcon(app).then((u) => (img.src = u));
    t.title = basename(app);
    t.style.top = TILE * (i + 1) + "px";
    pressable(t, () => {
      t.classList.add("pressed");
      setTimeout(() => t.classList.remove("pressed"), 150);
      call("launch_app", { app }).then(() => log(`launched ${basename(app)}`), (e) => alert(this.screen, { message: `Cannot launch ${basename(app)}`, detail: String(e) }));
    }, { drag: (e) => this.dragTile(t, app, i, e) });
    dropTarget(t, (paths) => paths.forEach((p) =>
      call("open_with", { app, path: p }).then(() => log(`opened ${basename(p)} with ${basename(app)}`), (e) => alert(this.screen, { message: "Cannot open", detail: String(e) }))));
    return t;
  }

  /** Vertical drag reorders; more than 48 px sideways removes (§7.4). Workspace and Recycler tiles are fixed. */
  private dragTile(t: HTMLElement, app: string, i: number, e: MouseEvent) {
    const top0 = TILE * (i + 1);
    let dx = 0, dy = 0;
    t.classList.add("dragging");
    trackDrag(e, {
      threshold: 0,
      move: (x, y) => { dx = x; dy = y; t.style.left = dx + "px"; t.style.top = top0 + dy + "px"; },
      up: () => {
        t.classList.remove("dragging");
        if (Math.abs(dx) > 48) { this.state.dock.splice(i, 1); log(`removed ${basename(app)} from the Dock`); }
        else {
          const j = Math.max(0, Math.min(this.state.dock.length - 1, Math.round((top0 + dy) / TILE) - 1));
          if (j !== i) { this.state.dock.splice(i, 1); this.state.dock.splice(j, 0, app); }
        }
        dirty(); this.render();
      },
    });
  }

  add(p: string) {
    if (this.state.dock.includes(p) || this.state.dock.length >= 12) return;
    this.state.dock.push(p); dirty(); this.render();
    log(`added ${basename(p)} to the Dock`);
  }

  async trash(paths: string[]) {
    try { await call("trash_paths", { paths }); log(`moved ${paths.length} item(s) to the Recycler`); }
    catch (e) { await alert(this.screen, { message: "Cannot move to the Recycler", detail: String(e) }); }
    this.hooks.fsChanged();
    this.updateTrash();
  }

  async updateTrash() {
    try { this.trashEmpty = await call<boolean>("trash_is_empty"); }
    catch (e) { if (!this.warned) { this.warned = true; log(`trash_is_empty: ${e}`); } this.trashEmpty = true; }
    this.recImg.src = icon[this.trashEmpty ? "recycler-empty" : "recycler-full"];
    if (this.recWin?.visible) this.fillRecycler();
  }

  /** §7.7: the only irreversible operation; always confirms. */
  async emptyRecycler() {
    if (await alert(this.screen, { message: "Are you sure you want to empty the Recycler?", detail: "Its contents will be permanently deleted.", buttons: ["Cancel", "Empty"] }) !== 1) return;
    try { await call("empty_trash"); log("emptied the Recycler"); }
    catch (e) { await alert(this.screen, { message: "Cannot empty the Recycler", detail: String(e) }); }
    this.updateTrash();
  }

  openRecycler() {
    const g = this.state.windows.recycler;
    if (!this.recWin) {
      this.recWin = new NXWindow(this.screen, { title: "Recycler", x: g.x, y: g.y, w: g.w, h: g.h, minW: 200, minH: 120, icon: icon["recycler-empty"],
        onMove: () => { g.x = this.recWin!.x; g.y = this.recWin!.y; dirty(); },
        onResize: () => { g.w = this.recWin!.w; g.h = this.recWin!.h; dirty(); },
        onClose: () => { g.open = false; dirty(); } });
      this.recScroller = new Scroller(this.recBody, true);
      this.recWin.content.append(el("div", "rec-row", this.recBody, this.recScroller.el));
    }
    this.recWin.show(); g.open = true; dirty();
    this.fillRecycler();
  }

  private button(label: string, action: () => void) {
    const b = el("div", "nx-raised nx-button", label);
    pressable(b, action);
    return b;
  }

  private async fillRecycler() {
    if (!isMac) { // Windows: the Recycle Bin is not a folder (documented limitation)
      this.recBody.replaceChildren(el("div", "rec-msg", this.button("Open Recycle Bin", () => call("open_recycle_bin").catch((e) => log(`open_recycle_bin: ${e}`)))));
      return;
    }
    try {
      const items = await call<Entry[]>("trash_list");
      this.recBody.replaceChildren(...items.map((e) => {
        const img = el("img"); img.src = iconFor(e.name, e.is_dir, e.is_app);
        return el("div", "rec-cell", img, el("div", "lbl nx-small nx-ellipsis", e.name));
      }));
      if (!items.length) this.recBody.append(el("div", "rec-msg", "The Recycler is empty."));
    } catch (e) {
      this.recBody.replaceChildren(el("div", "rec-msg",
        el("div", "", "Cannot list the Recycler."), el("div", "nx-small", String(e)),
        el("div", "nx-small", "Grant ReWorkspace Full Disk Access in System Settings ▸ Privacy & Security, or open it in the Finder:"),
        this.button("Open in Finder", () => call("open_path", { path: join(this.home, ".Trash") }).catch((err) => log(`open .Trash: ${err}`)))));
    }
    this.recScroller?.sync();
  }
}
