import { el, triRight, truncMiddle, cmdKey } from "../../chrome/ui";
import { Screen, trackDrag, pressable } from "../../screen/screen";
import { NXWindow } from "../../chrome/window";
import { Scroller } from "../../chrome/scroller";
import { alert } from "../../chrome/alert";
import { call, Entry } from "../../backend";
import { icon, iconFor, smallIconFor, ext } from "../../icons";
import { State, dirty } from "../../state";
import { ancestors, basename, dirname, isRoot } from "../../paths";
import { startDrag, dropTarget } from "../../screen/dnd";
import { log } from "./panels";

const CELL = 18;

interface Column {
  dir: string | null; // null = synthetic "Computer" column (Windows drive list)
  el: HTMLElement; list: HTMLElement; scroller: Scroller;
  entries: Entry[]; selected: Set<string>; anchor: number; seq: number;
}

const sig = (v: Entry[]) => JSON.stringify(v.map((e) => [e.name, e.is_dir, e.size, e.modified]));

/** Guess an icon for a bare path (Shelf items are stored as paths only). */
export function pathIcon(p: string, home: string) {
  if (p === home) return icon["home"];
  if (isRoot(p)) return icon["drive"];
  const n = basename(p), e = ext(n);
  return e === "app" || e === "exe" || e === "lnk" ? icon["application"] : e ? iconFor(n, false) : icon["folder"]; // ponytail: extension heuristic, no stat
}

/** The Workspace File Viewer (§7.8): Shelf, Icon Path, column Browser. */
export class FileViewer {
  win: NXWindow;
  shelfEl: HTMLElement; pathEl: HTMLElement; colsEl: HTMLElement; hscroll: Scroller;
  cols: Column[] = [];
  focus = 0;
  onSelection: (sel: Entry[], dir: string) => void = () => {};
  onFsChange: () => void = () => {};
  private typeahead = ""; private typeTimer = 0; private navSeq = 0;
  private lastClick = { path: "", t: 0 };

  constructor(public screen: Screen, public state: State, public home: string, public roots: string[]) {
    const g = state.windows.file_viewer;
    this.win = new NXWindow(screen, {
      title: "File Viewer", x: g.x, y: g.y, w: g.w, h: g.h, minW: 480, minH: 320, icon: icon["folder"],
      onMove: () => { g.x = this.win.x; g.y = this.win.y; dirty(); },
      onResize: () => { g.w = this.win.w; g.h = this.win.h; dirty(); },
      onClose: () => { g.open = false; dirty(); },
      onKeyDown: (e) => this.onKey(e),
    });
    this.shelfEl = el("div", "nx-raised shelf");
    this.pathEl = el("div", "iconpath");
    this.colsEl = el("div", "br-cols");
    this.hscroll = new Scroller(this.colsEl, false);
    this.win.content.classList.add("fv-content");
    this.win.content.append(this.shelfEl, this.pathEl, el("div", "nx-sunken br-area", this.colsEl, this.hscroll.el));
    dropTarget(this.shelfEl, (paths) => paths.forEach((p) => this.addShelf(p)));
    this.renderShelf();
  }

  // ---- Shelf
  renderShelf() {
    this.shelfEl.replaceChildren(...this.state.shelf.slice(0, 16).map((p) => {
      const img = el("img"); img.src = pathIcon(p, this.home);
      const item = el("div", "shelf-item", img, el("div", "lbl nx-small nx-ellipsis", truncMiddle(basename(p), 11)));
      item.title = p;
      pressable(item, () => this.navigate(p), {
        drag: (e) => startDrag(this.screen, e, [p], img.src, { dispatch: false, onEnd: (_t, ev) => {
          if (!this.shelfEl.contains(document.elementFromPoint(ev.clientX, ev.clientY))) this.removeShelf(p);
        } }),
      });
      return item;
    }));
  }
  addShelf(p: string) {
    if (this.state.shelf.includes(p) || this.state.shelf.length >= 16) return;
    this.state.shelf.push(p); dirty(); this.renderShelf();
  }
  removeShelf(p: string) { this.state.shelf = this.state.shelf.filter((x) => x !== p); dirty(); this.renderShelf(); }

  // ---- Icon Path
  private renderPath() {
    const e = this.selEntry;
    const chain = ancestors(this.selPath).filter((x): x is string => x !== null);
    this.pathEl.replaceChildren(...chain.map((c, i) => {
      const last = i === chain.length - 1;
      const img = el("img");
      img.src = c === this.home ? icon["home"] : isRoot(c) ? icon["drive"] : last && e && !e.is_dir ? iconFor(e.name, false, e.is_app) : icon["folder"];
      const item = el("div", "ip-item", img, el("div", "ip-label nx-small nx-ellipsis", truncMiddle(basename(c), 11)));
      item.title = c;
      pressable(item, () => this.navigate(c));
      return item;
    }));
  }

  // ---- Columns
  private addColumn(dir: string | null): Column {
    const list = el("div", "br-list");
    const scroller = new Scroller(list, true);
    const col: Column = { dir, el: el("div", "br-col", list, scroller.el), list, scroller, entries: [], selected: new Set(), anchor: -1, seq: 0 };
    dropTarget(list, (paths, e) => this.dropInto(col, paths, e));
    this.colsEl.append(col.el);
    this.cols.push(col);
    return col;
  }
  private truncate(n: number) {
    while (this.cols.length > n) { const c = this.cols.pop()!; c.scroller.destroy(); c.el.remove(); }
    this.focus = Math.min(this.focus, Math.max(0, this.cols.length - 1));
  }
  private scrollToEnd() { this.colsEl.scrollLeft = this.colsEl.scrollWidth; this.hscroll.sync(); }

  private async load(col: Column, quiet = false): Promise<boolean> {
    const seq = ++col.seq;
    if (col.dir === null) {
      col.entries = this.roots.map((r) => ({ name: r, path: r, is_dir: true, is_symlink: false, is_app: false, size: 0, modified: 0, hidden: false }));
      this.render(col); return true;
    }
    try {
      const entries = await call<Entry[]>("list_dir", { path: col.dir, showHidden: this.state.show_hidden });
      if (seq !== col.seq) return false;
      col.entries = entries; this.render(col); return true;
    } catch (e) {
      if (seq !== col.seq) return false;
      col.entries = [];
      col.list.replaceChildren(el("div", "br-unreadable", "(unreadable)"));
      log(`list_dir failed: ${e}`);
      if (!quiet) alert(this.screen, { message: "Cannot read folder", detail: String(e) });
      return false;
    }
  }

  private render(col: Column) {
    const avail = (col.list.clientWidth || 140) - 36;
    col.list.replaceChildren(...col.entries.map((e, i) => this.cell(col, e, i, Math.max(8, Math.floor(avail / 6.3)))));
    this.applySel(col); this.syncCol(col);
  }
  private cell(col: Column, e: Entry, i: number, maxChars: number) {
    const img = el("img", "ic"); img.src = smallIconFor(e.name, e.is_dir, e.is_app);
    const c = el("div", "br-cell", img, el("span", "nm", truncMiddle(e.name, maxChars)));
    if (e.is_symlink) { const b = el("img", "badge"); b.src = icon["symlink-badge"]; c.append(b); }
    if (e.is_dir) c.append(triRight(6));
    c.title = e.name;
    c.addEventListener("mousedown", (ev) => this.cellDown(col, e, i, ev));
    return c;
  }
  private applySel(col: Column) {
    Array.from(col.list.children).forEach((c, i) => c.classList.toggle("sel", !!col.entries[i] && col.selected.has(col.entries[i].path)));
  }
  private syncCol(col: Column) { col.scroller.sync(); col.scroller.el.style.display = col.scroller.overflowing ? "" : "none"; }
  private ensureVisible(col: Column, i: number) {
    const top = i * CELL, l = col.list;
    if (top < l.scrollTop) l.scrollTop = top;
    else if (top + CELL > l.scrollTop + l.clientHeight) l.scrollTop = top + CELL - l.clientHeight;
  }

  selection(col: Column = this.cols[this.focus]) { return col ? col.entries.filter((e) => col.selected.has(e.path)) : []; }
  /** Selection of the deepest column that has one (what the title, Icon Path and Inspector follow). */
  get deepSelection(): Entry[] {
    for (let i = this.cols.length - 1; i >= 0; i--) { const s = this.selection(this.cols[i]); if (s.length) return s; }
    return [];
  }
  get selEntry(): Entry | null { return this.deepSelection[0] ?? null; }
  get selPath(): string { return this.selEntry?.path ?? this.cols[this.cols.length - 1]?.dir ?? this.roots[0]; }
  get currentDir(): string {
    const e = this.selEntry;
    if (!e) return this.cols[this.cols.length - 1]?.dir ?? this.roots[0];
    return e.is_dir ? e.path : dirname(e.path) ?? e.path;
  }

  private cellDown(col: Column, e: Entry, i: number, ev: MouseEvent) {
    if (ev.button !== 0) return;
    ev.stopPropagation(); ev.preventDefault();
    const was = col.selected.has(e.path);
    if (ev.shiftKey && col.anchor >= 0) {
      const [a, b] = [Math.min(col.anchor, i), Math.max(col.anchor, i)];
      col.selected = new Set(col.entries.slice(a, b + 1).map((x) => x.path));
    } else if (cmdKey(ev)) {
      if (was) col.selected.delete(e.path); else col.selected.add(e.path);
      col.anchor = i;
    } else if (!was || col.selected.size === 1) {
      col.selected = new Set([e.path]); col.anchor = i;
    } // else: press on an already multi-selected cell keeps the selection (drag start)
    this.focus = this.cols.indexOf(col);
    this.applySel(col);
    this.afterSelect(col);
    const now = performance.now();
    const dbl = this.lastClick.path === e.path && now - this.lastClick.t < 400;
    this.lastClick = { path: e.path, t: dbl ? 0 : now };
    if (dbl && !ev.shiftKey && !cmdKey(ev)) { if (!e.is_dir) this.open([e]); return; }
    let started = false;
    trackDrag(ev, { move: (_x, _y, mv) => {
      if (started) return;
      started = true;
      const sel = this.selection(col);
      if (sel.length) startDrag(this.screen, mv, sel.map((s) => s.path), iconFor(sel[0].name, sel[0].is_dir, sel[0].is_app));
    } });
  }

  private afterSelect(col: Column) {
    const idx = this.cols.indexOf(col);
    const sel = this.selection(col);
    this.truncate(idx + 1);
    this.focus = idx;
    if (sel.length === 1 && sel[0].is_dir) { this.load(this.addColumn(sel[0].path)); this.scrollToEnd(); }
    this.changed();
  }

  private changed() {
    const dir = this.currentDir;
    this.win.setTitle(`File Viewer — ${dir}`);
    this.renderPath();
    this.state.windows.file_viewer.path = this.selPath; dirty();
    this.onSelection(this.deepSelection, dir);
  }

  /** Select `path`, opening columns from the root down to it. */
  async navigate(path: string) {
    const nav = ++this.navSeq;
    const chain = ancestors(path);
    this.truncate(0);
    for (let i = 0; i < chain.length; i++) {
      const col = this.addColumn(chain[i]);
      const ok = await this.load(col);
      if (nav !== this.navSeq) return;
      if (!ok) break;
      const next = chain[i + 1];
      if (next === undefined) break;
      const e = col.entries.find((x) => x.path === next);
      if (!e) break;
      col.selected = new Set([e.path]); col.anchor = col.entries.indexOf(e); this.focus = i;
      this.applySel(col); this.ensureVisible(col, col.anchor);
      if (!e.is_dir) break;
    }
    this.scrollToEnd(); this.changed();
  }

  /** Re-list visible columns, preserving selection by path (§9.13). */
  async refresh() {
    for (const col of [...this.cols]) {
      if (col.dir === null) continue;
      const seq = ++col.seq;
      let entries: Entry[];
      try { entries = await call<Entry[]>("list_dir", { path: col.dir, showHidden: this.state.show_hidden }); } catch { continue; }
      if (seq !== col.seq || !this.cols.includes(col) || sig(entries) === sig(col.entries)) continue;
      const top = col.list.scrollTop;
      col.entries = entries;
      col.selected = new Set([...col.selected].filter((p) => entries.some((e) => e.path === p)));
      this.render(col); col.list.scrollTop = top;
      const next = this.cols[this.cols.indexOf(col) + 1];
      if (next && !entries.some((e) => e.path === next.dir)) { this.truncate(this.cols.indexOf(col) + 1); this.changed(); }
    }
    this.onFsChange();
  }

  // ---- keyboard (§7.8)
  private onKey(e: KeyboardEvent): boolean {
    if (cmdKey(e) || e.altKey) return false;
    const col = this.cols[this.focus];
    if (!col) return false;
    const sel = this.selection(col);
    const cur = sel.length ? col.entries.indexOf(sel[sel.length - 1]) : -1;
    const pick = (col: Column, i: number) => {
      i = Math.max(0, Math.min(i, col.entries.length - 1));
      if (i < 0) return;
      col.selected = new Set([col.entries[i].path]); col.anchor = i;
      this.applySel(col); this.ensureVisible(col, i); this.afterSelect(col);
    };
    switch (e.key) {
      case "ArrowDown": pick(col, cur + 1); return true;
      case "ArrowUp": pick(col, cur < 0 ? 0 : cur - 1); return true;
      case "ArrowRight": { const n = this.cols[this.focus + 1]; if (n && n.entries.length) pick(n, 0); return true; }
      case "ArrowLeft":
        if (this.focus > 0) { this.focus--; this.cols.slice(this.focus + 1).forEach((c) => { c.selected.clear(); this.applySel(c); }); this.changed(); }
        return true;
      case "Enter": this.open(sel); return true;
      case "Backspace": case "Delete": this.trashSelection(true); return true;
    }
    if (e.key.length === 1) {
      this.typeahead += e.key.toLowerCase();
      clearTimeout(this.typeTimer);
      this.typeTimer = window.setTimeout(() => (this.typeahead = ""), 700);
      const j = col.entries.findIndex((x) => x.name.toLowerCase().startsWith(this.typeahead));
      if (j >= 0) pick(col, j);
      return true;
    }
    return false;
  }

  // ---- actions
  open(entries: Entry[] = this.deepSelection) {
    for (const e of entries) if (!e.is_dir) { log(`open ${e.path}`); call("open_path", { path: e.path }).catch((err) => alert(this.screen, { message: "Cannot open", detail: String(err) })); }
  }
  async trashSelection(confirm: boolean) {
    const sel = this.deepSelection;
    if (!sel.length) return;
    if (confirm) {
      const what = sel.length === 1 ? `“${sel[0].name}”` : `${sel.length} items`;
      if (await alert(this.screen, { message: `Move ${what} to the Recycler?`, buttons: ["Cancel", "Move"] }) !== 1) return;
    }
    await this.run("trash_paths", { paths: sel.map((e) => e.path) }, `trash ${sel.length} item(s)`);
  }
  async destroySelection() {
    const sel = this.deepSelection;
    if (!sel.length) return;
    const what = sel.length === 1 ? `“${sel[0].name}”` : `${sel.length} items`;
    if (await alert(this.screen, { message: `Destroy ${what}?`, detail: "Destroy permanently deletes the file. Continue?", buttons: ["Cancel", "Destroy"] }) !== 1) return;
    await this.run("destroy_paths", { paths: sel.map((e) => e.path) }, `destroy ${sel.length} item(s)`);
  }
  async newFolder() {
    const dir = this.currentDir;
    try { const p = await call<string>("new_folder", { parent: dir }); log(`new folder ${p}`); await this.navigate(p); this.onFsChange(); }
    catch (e) { alert(this.screen, { message: "Cannot create folder", detail: String(e) }); }
  }
  async duplicate() {
    const sel = this.deepSelection;
    if (sel.length) await this.run("duplicate_paths", { paths: sel.map((e) => e.path) }, `duplicate ${sel.length} item(s)`);
  }
  async paste(paths: string[]) {
    if (paths.length) await this.run("copy_paths", { paths, destDir: this.currentDir }, `paste ${paths.length} item(s) into ${this.currentDir}`);
  }
  selectAll() {
    const col = this.cols[this.focus];
    if (!col) return;
    col.selected = new Set(col.entries.map((e) => e.path));
    this.applySel(col); this.afterSelect(col);
  }
  /** Run a mutating command, then refresh; errors go to an alert. */
  async run(cmd: string, args: Record<string, unknown>, what: string) {
    try { await call(cmd, args); log(what); }
    catch (e) { log(`${what} failed: ${e}`); await alert(this.screen, { message: "Operation failed", detail: String(e) }); }
    await this.refresh();
  }

  private dropInto(col: Column, paths: string[], e: MouseEvent) {
    const cellEl = (e.target as HTMLElement).closest(".br-cell");
    const idx = cellEl ? Array.from(col.list.children).indexOf(cellEl) : -1;
    const target = idx >= 0 && col.entries[idx]?.is_dir ? col.entries[idx].path : col.dir;
    if (!target || paths.some((p) => p === target || target.startsWith(p + "/") || target.startsWith(p + "\\") || dirname(p) === target)) return;
    const copy = e.altKey;
    this.run(copy ? "copy_paths" : "move_paths", { paths, destDir: target }, `${copy ? "copy" : "move"} ${paths.length} item(s) to ${target}`);
  }
}
