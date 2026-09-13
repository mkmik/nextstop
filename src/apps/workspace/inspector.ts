import { el, svg, truncMiddle } from "../../chrome/ui";
import { Screen, pressable } from "../../screen/screen";
import { NXWindow } from "../../chrome/window";
import { Scroller } from "../../chrome/scroller";
import { call, Entry, Meta } from "../../backend";
import { icon, iconFor, kindFor, fileIconName, ext } from "../../icons";
import { State, dirty } from "../../state";

const MODES = ["Attributes", "Contents", "Tools", "Access Control"] as const;
type Mode = (typeof MODES)[number];
const fmtDate = (s: number | null) => s == null ? "—" : new Date(s * 1000).toISOString().replace("T", " ").slice(0, 16);
const fmtSize = (n: number) => n.toLocaleString("en-US") + " bytes";
const IMG_MIME: Record<string, string> = { png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", gif: "image/gif", bmp: "image/bmp", webp: "image/webp", svg: "image/svg+xml" };

/** Inspector panel (§7.9): Attributes and Contents of the current selection. */
export class Inspector {
  win: NXWindow;
  mode: Mode = "Attributes";
  entry: Entry | null = null;
  private head = el("div", "insp-head");
  private body = el("div", "insp-body");
  private modeLbl = el("span", "lbl", this.mode);
  private seq = 0;

  constructor(public screen: Screen, state: State) {
    const g = state.windows.inspector;
    this.win = new NXWindow(screen, { title: "Inspector", x: g.x, y: g.y, w: 272, h: 400, resizable: false, icon: icon["miniwindow"],
      onMove: () => { g.x = this.win.x; g.y = this.win.y; dirty(); }, onClose: () => { g.open = false; dirty(); } });
    const popup = el("div", "nx-raised nx-button insp-popup", this.modeLbl, svg(7, 12, `<path d="M0 5L3.5 0L7 5Z M0 7L7 7L3.5 12Z" fill="#000"/>`));
    pressable(popup, () => this.openPopup(popup));
    this.win.content.append(this.head, popup, this.body);
    this.render();
  }

  private openPopup(btn: HTMLElement) {
    const list = el("div", "insp-popup-list");
    for (const m of MODES) {
      const disabled = m === "Tools" || m === "Access Control";
      const row = el("div", "nx-raised nx-menu-item" + (disabled ? " disabled" : "") + (m === this.mode ? " checked" : ""), el("span", "label", m));
      if (!disabled) pressable(row, () => { this.mode = m; this.modeLbl.textContent = m; close(); this.render(); }, { pressedClass: "hi" });
      list.append(row);
    }
    list.style.top = btn.offsetTop + btn.offsetHeight + "px";
    const close = () => { list.remove(); document.removeEventListener("mousedown", onDoc, true); };
    const onDoc = (e: MouseEvent) => { if (!list.contains(e.target as Node)) close(); };
    document.addEventListener("mousedown", onDoc, true);
    this.win.content.append(list);
  }

  update(e: Entry | null) {
    if (e?.path === this.entry?.path && e?.modified === this.entry?.modified) return;
    this.entry = e;
    this.render();
  }

  private render() {
    const e = this.entry, seq = ++this.seq;
    this.head.replaceChildren();
    this.body.replaceChildren();
    if (!e) { this.body.append(el("div", "insp-msg", "No selection")); return; }
    const img = el("img"); img.src = iconFor(e.name, e.is_dir, e.is_app);
    this.head.append(img, el("div", "nx-bold nx-ellipsis", truncMiddle(e.name, 28)));
    if (this.mode === "Attributes") this.renderAttrs(e, seq);
    else this.renderContents(e, seq);
  }

  private async renderAttrs(e: Entry, seq: number) {
    let m: Meta | null = null;
    try { m = await call<Meta>("file_meta", { path: e.path }); } catch { /* shown as unavailable */ }
    if (seq !== this.seq) return;
    const grid = el("div", "insp-grid");
    const row = (k: string, v: Node | string) => grid.append(el("div", "k", k), typeof v === "string" ? el("div", "v nx-ellipsis", v) : v);
    row("Path", e.path);
    row("Kind", e.is_symlink ? "Symbolic link" : kindFor(e.name, e.is_dir, e.is_app));
    if (e.is_dir) {
      const v = el("div", "v", "—");
      const b = el("div", "nx-raised nx-button insp-compute", "Compute");
      pressable(b, async () => { b.textContent = "…"; try { const n = await call<number>("dir_size", { path: e.path }); v.textContent = fmtSize(n); } catch { v.textContent = "?"; } b.remove(); });
      row("Size", el("div", "v insp-size", v, b));
    } else row("Size", fmtSize(m?.size ?? e.size));
    row("Modified", fmtDate(m?.modified ?? e.modified));
    row("Created", fmtDate(m?.created ?? null));
    row("Permissions", m?.mode ?? "—");
    if (m?.owner != null) row("Owner", m.owner);
    this.body.replaceChildren(grid);
  }

  private async renderContents(e: Entry, seq: number) {
    if (e.is_dir) { this.body.append(el("div", "insp-msg", "No contents viewer for folders")); return; }
    const kind = fileIconName(e.name), x = ext(e.name);
    if (kind === "file-image" && IMG_MIME[x]) {
      try {
        const b64 = await call<string>("read_file_b64", { path: e.path, maxBytes: 32 * 1024 * 1024 });
        if (seq !== this.seq) return;
        const img = el("img", "insp-img"); img.src = `data:${IMG_MIME[x]};base64,${b64}`;
        this.body.append(img);
      } catch (err) { if (seq === this.seq) this.body.append(el("div", "insp-msg", String(err))); }
      return;
    }
    if ((kind === "file-text" || kind === "file-code") && e.size < 256 * 1024) {
      try {
        const text = await call<string>("read_text_head", { path: e.path, maxBytes: 256 * 1024 });
        if (seq !== this.seq) return;
        const view = el("div", "nx-sunken insp-text nx-mono", text.split("\n").slice(0, 200).join("\n"));
        const sc = new Scroller(view, true);
        this.body.append(el("div", "insp-textrow", view, sc.el));
        sc.sync();
      } catch (err) { if (seq === this.seq) this.body.append(el("div", "insp-msg", String(err))); }
      return;
    }
    this.body.append(el("div", "insp-msg", "No contents viewer for this type"));
  }
}
