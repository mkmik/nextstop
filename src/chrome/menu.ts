import { el, svg, triRight, cmdKey } from "./ui";
import { Screen, trackDrag, pressable } from "../screen/screen";

export interface MenuItem {
  label: string;
  key?: string;                       // key equivalent letter (case-sensitive)
  disabled?: boolean | (() => boolean);
  checked?: () => boolean;
  action?: () => void;
  submenu?: MenuItem[];
  gapBefore?: boolean;                // 2 px dark line above (main menu: before "Hide")
}

export interface MenuOpts {
  x: number; y: number;
  main?: boolean;                     // fixed 130 px wide
  transient?: boolean;                // right-click popup: destroyed after use
  parent?: Menu;
  path?: string[];                    // ["View", "Scale"], used for torn-off persistence
  onMove?: (m: Menu) => void;
  onTear?: (m: Menu) => void;
  onCloseTorn?: (m: Menu) => void;
}

const isDisabled = (it: MenuItem) => (typeof it.disabled === "function" ? it.disabled() : !!it.disabled);
const closeGlyph = () => svg(14, 14, `<path d="M3.5 3.5L10.5 10.5M10.5 3.5L3.5 10.5" stroke="#000" stroke-width="1"/>`);

/** Vertical NeXT menu (§7.3). Submenus open to the right; drag a submenu's title to tear it off. */
export class Menu {
  static all = new Set<Menu>();
  static torns = new Map<string, Menu>();
  el: HTMLElement;
  titleEl: HTMLElement;
  child: Menu | null = null;
  childItem: HTMLElement | null = null;
  torn = false;
  x: number; y: number;
  private rows: [MenuItem, HTMLElement][] = [];

  constructor(public screen: Screen, public title: string, public items: MenuItem[], public opts: MenuOpts) {
    this.x = opts.x; this.y = opts.y;
    this.titleEl = el("div", "nx-menu-title", el("span", "nx-ellipsis", title));
    this.el = el("div", "nx-menu", this.titleEl);
    for (const it of items) {
      if (it.gapBefore) this.el.append(el("div", "nx-menu-gap"));
      this.el.append(this.buildItem(it));
    }
    this.titleEl.addEventListener("mousedown", (e) => this.onTitleDown(e));
    this.el.addEventListener("mousedown", (e) => e.stopPropagation()); // clicks inside never reach the Screen
    screen.menus.append(this.el);
    // width: main menu 130; submenus widest item + 24, min 110 (shrink-to-fit width already includes padding)
    this.el.style.width = opts.main ? "130px" : Math.max(110, this.el.offsetWidth + 2) + "px";
    this.apply();
    screen.front(this);
    Menu.all.add(this);
  }

  get key() { return (this.opts.path ?? []).join("/"); }

  private buildItem(it: MenuItem) {
    const row = el("div", "nx-raised nx-menu-item", el("span", "label nx-ellipsis", it.label));
    const right = el("span", "right");
    if (it.submenu) right.append(triRight(6));
    else if (it.key) right.append(el("span", "nx-cmd"), el("span", "", it.key));
    row.append(right);
    this.rows.push([it, row]);
    row.addEventListener("mousedown", (e) => {
      if (e.button !== 0) return;
      e.stopPropagation(); e.preventDefault();
      if (isDisabled(it)) return;
      if (it.submenu) { this.toggleChild(it, row); return; }
      row.classList.add("hi");
      trackDrag(e, {
        up: (ev) => {
          row.classList.remove("hi");
          if (!row.contains(document.elementFromPoint(ev.clientX, ev.clientY))) return;
          Menu.closeAll();
          it.action?.();
          Menu.refreshAll();
        },
      });
    });
    return row;
  }

  refresh() {
    for (const [it, row] of this.rows) {
      row.classList.toggle("disabled", isDisabled(it));
      row.classList.toggle("checked", !!it.checked?.());
    }
  }
  static refreshAll() { Menu.all.forEach((m) => m.refresh()); }

  private toggleChild(it: MenuItem, row: HTMLElement) {
    if (this.child && this.childItem === row) { this.closeChild(); return; }
    this.closeChild();
    const path = [...(this.opts.path ?? []), it.label];
    const torn = Menu.torns.get(path.join("/"));
    if (torn) { this.screen.front(torn); return; } // already torn off: just bring it forward
    row.classList.add("hi");
    this.childItem = row;
    this.child = new Menu(this.screen, it.label, it.submenu!, {
      x: 0, y: 0, parent: this, path, transient: this.opts.transient,
      onMove: this.opts.onMove, onTear: this.opts.onTear, onCloseTorn: this.opts.onCloseTorn,
    });
    this.child.reattach();
    this.child.refresh();
  }

  closeChild() {
    if (!this.child) return;
    this.child.destroy();
    this.child = null;
    this.childItem?.classList.remove("hi");
    this.childItem = null;
  }

  /** Position an attached submenu right of its parent, top-aligned with the item. */
  reattach() {
    const p = this.opts.parent;
    if (!p || !p.childItem) return;
    this.x = p.x + p.el.offsetWidth; this.y = p.y + p.childItem.offsetTop;
    this.apply();
    this.child?.reattach();
  }

  private onTitleDown(e: MouseEvent) {
    if (e.button !== 0) return;
    e.stopPropagation(); e.preventDefault();
    this.screen.front(this);
    const x0 = this.x, y0 = this.y;
    trackDrag(e, {
      move: (dx, dy) => {
        if (!this.torn && !this.opts.main) this.tearOff();
        this.x = Math.round(x0 + dx); this.y = Math.round(y0 + dy);
        this.apply();
        this.child?.reattach();
      },
      up: (_e, moved) => { if (moved) this.opts.onMove?.(this); },
    });
  }

  tearOff() {
    if (this.torn) return;
    this.torn = true;
    this.opts.transient = false;
    const p = this.opts.parent;
    if (p) { p.child = null; p.childItem?.classList.remove("hi"); p.childItem = null; }
    this.opts.parent = undefined;
    const b = el("div", "nx-raised nx-tb-btn close", closeGlyph());
    pressable(b, () => { this.destroy(); this.opts.onCloseTorn?.(this); });
    this.titleEl.append(b);
    Menu.torns.set(this.key, this);
    this.opts.onTear?.(this);
  }

  apply() { this.el.style.left = this.x + "px"; this.el.style.top = this.y + "px"; }

  destroy() {
    this.closeChild();
    this.el.remove();
    Menu.all.delete(this);
    if (this.torn) Menu.torns.delete(this.key);
  }

  /** Close every attached submenu; destroy transient popups. Torn-off menus stay (§9.7). */
  static closeAll() {
    for (const m of [...Menu.all]) {
      if (m.opts.parent) continue;
      if (m.opts.transient && !m.torn) m.destroy(); else m.closeChild();
    }
  }

  /** Re-create a torn-off menu from a saved path (§7.3 persistence). */
  static openTorn(screen: Screen, root: MenuItem[], path: string[], x: number, y: number, opts: Partial<MenuOpts>) {
    let items = root;
    for (const label of path) {
      const it = items.find((i) => i.label === label);
      if (!it?.submenu) return null;
      items = it.submenu;
    }
    const m = new Menu(screen, path[path.length - 1], items, { x, y, path, ...opts });
    m.tearOff();
    m.refresh();
    return m;
  }

  /** Key equivalents (§7.3): Cmd (macOS) / Ctrl (Windows) + letter. */
  static handleKey(items: MenuItem[], e: KeyboardEvent): boolean {
    if (!cmdKey(e) || e.altKey) return false;
    const walk = (list: MenuItem[]): boolean => list.some((it) =>
      it.submenu ? walk(it.submenu) : it.key === e.key && !isDisabled(it) ? (Menu.closeAll(), it.action?.(), Menu.refreshAll(), true) : false);
    return walk(items);
  }
}

// §9.7: clicking anywhere outside a menu closes attached submenus.
document.addEventListener("mousedown", (e) => {
  if (!(e.target as Element).closest?.(".nx-menu")) Menu.closeAll();
}, true);
