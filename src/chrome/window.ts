import { el, svg, truncMiddle } from "./ui";
import { Screen, trackDrag, pressable, scale } from "../screen/screen";

export interface WinOpts {
  title: string;
  x: number; y: number; w: number; h: number;
  minW?: number; minH?: number;
  resizable?: boolean;      // default true
  miniaturizable?: boolean; // default true
  closable?: boolean;       // default true
  icon?: string;            // miniwindow icon URL
  layer?: HTMLElement;      // default: screen.windows
  onClose?: () => void;     // default: hide
  onMove?: () => void;
  onResize?: () => void;
  onKey?: (key: boolean) => void;
  onKeyDown?: (e: KeyboardEvent) => boolean | void; // return true if handled
}

export let animations = true;
export const setAnimations = (on: boolean) => { animations = on; };

const TITLE_H = 22, RESIZE_H = 8;

export class NXWindow {
  el: HTMLElement;
  content: HTMLElement;
  titleEl: HTMLElement;
  key = false;
  visible = false;
  mini: HTMLElement | null = null;
  opts: WinOpts;
  x: number; y: number; w: number; h: number;

  constructor(public screen: Screen, opts: WinOpts) {
    this.opts = opts;
    this.x = opts.x; this.y = opts.y; this.w = opts.w; this.h = opts.h;
    const resizable = opts.resizable ?? true;

    this.titleEl = el("div", "nx-tb-title nx-ellipsis", opts.title);
    const tb = el("div", "nx-titlebar", this.titleEl);
    if (opts.miniaturizable ?? true) {
      const b = el("div", "nx-raised nx-tb-btn mini", svg(14, 14, `<rect x="4" y="6" width="6" height="2" fill="#000"/>`));
      pressable(b, () => this.miniaturize());
      tb.append(b);
    }
    if (opts.closable ?? true) {
      const b = el("div", "nx-raised nx-tb-btn close", svg(14, 14, `<path d="M3.5 3.5L10.5 10.5M10.5 3.5L3.5 10.5" stroke="#000" stroke-width="1"/>`));
      pressable(b, () => this.close());
      tb.append(b);
    }
    tb.addEventListener("mousedown", (e) => {
      if (e.button !== 0) return;
      const x0 = this.x, y0 = this.y;
      trackDrag(e, { move: (dx, dy) => this.moveTo(x0 + dx, y0 + dy) });
    });

    this.content = el("div", "nx-content");
    this.el = el("div", "nx-window", tb, this.content);
    if (resizable) this.el.append(this.buildResizeBar());
    this.el.addEventListener("mousedown", () => this.screen.makeKey(this), true);
    (opts.layer ?? screen.windows).append(this.el);
    this.el.style.display = "none";
    this.apply();
    screen.register(this);
  }

  private buildResizeBar() {
    const bar = el("div", "nx-raised nx-resizebar");
    const regions: [string, number, number][] = [["left", -1, 1], ["mid", 0, 1], ["right", 1, 1]];
    for (const [cls, dw, dh] of regions) {
      const r = el("div", "seg " + cls);
      r.addEventListener("mousedown", (e) => {
        if (e.button !== 0) return;
        e.stopPropagation();
        this.screen.makeKey(this);
        const x0 = this.x, w0 = this.w, h0 = this.h;
        const minW = this.opts.minW ?? 120, minH = this.opts.minH ?? 60;
        trackDrag(e, {
          threshold: 0,
          move: (dx, dy) => {
            let w = w0, x = x0;
            if (dw < 0) { w = Math.max(minW, w0 - dx); x = x0 + (w0 - w); }
            if (dw > 0) w = Math.max(minW, w0 + dx);
            const h = Math.max(minH, h0 + dy * dh);
            this.x = x; this.w = w; this.h = h;
            this.apply(); this.opts.onResize?.();
          },
        });
      });
      bar.append(r);
    }
    bar.append(el("div", "div1"), el("div", "div2"));
    return bar;
  }

  apply() {
    const s = this.el.style;
    s.left = this.x + "px"; s.top = this.y + "px"; s.width = this.w + "px"; s.height = this.h + "px";
  }

  moveTo(x: number, y: number) {
    // §7.2: the title bar must stay at least 20 px inside the Screen on every side.
    this.x = Math.round(Math.min(Math.max(x, 20 - this.w), this.screen.w - 20));
    this.y = Math.round(Math.min(Math.max(y, 20 - TITLE_H), this.screen.h - 20));
    this.apply(); this.opts.onMove?.();
  }

  resizeTo(w: number, h: number) {
    this.w = Math.max(this.opts.minW ?? 120, w); this.h = Math.max(this.opts.minH ?? 60, h);
    this.apply(); this.opts.onResize?.();
  }

  setTitle(t: string) { this.titleEl.textContent = t; }

  get contentHeight() { return this.h - TITLE_H - ((this.opts.resizable ?? true) ? RESIZE_H : 0); }

  show(makeKey = true) {
    if (this.mini) this.restore();
    this.visible = true;
    this.el.style.display = "";
    this.moveTo(this.x, this.y);
    if (makeKey) this.screen.makeKey(this); else this.screen.front(this);
  }

  hide() {
    this.visible = false;
    this.el.style.display = "none";
    if (this.key) this.screen.dropKey(this);
  }

  close() { this.hide(); this.opts.onClose?.(); }

  destroy() { this.hide(); this.mini?.remove(); this.el.remove(); this.screen.unregister(this); }

  setKey(k: boolean) {
    this.key = k;
    this.el.classList.toggle("key", k);
    this.opts.onKey?.(k);
  }

  /** §7.2 miniaturize: collapse into a 64×64 miniwindow tile on the bottom row. */
  miniaturize() {
    if (this.mini) return;
    const icon = this.opts.icon ? el("img", "", ) : el("div", "");
    if (this.opts.icon) (icon as HTMLImageElement).src = this.opts.icon;
    icon.className = "mw-icon";
    const mini = el("div", "nx-raised nx-miniwindow", icon, el("div", "mw-title nx-small nx-ellipsis", truncMiddle(this.opts.title, 12)));
    mini.title = this.opts.title;
    const slot = this.screen.tiles.querySelectorAll(".nx-miniwindow").length;
    mini.style.left = 64 * (slot + 1) + "px";
    let last = 0;
    mini.addEventListener("mousedown", (e) => {
      if (e.button !== 0) return;
      e.stopPropagation();
      const now = performance.now();
      if (now - last < 400) { last = 0; this.restore(); } else last = now;
    });
    this.mini = mini;
    const to = { x: 64 * (slot + 1), y: this.screen.h - 64, w: 64, h: 64 };
    this.animate(to, () => {
      this.visible = false;
      this.el.style.display = "none";
      this.screen.tiles.append(mini);
    });
    if (this.key) this.screen.dropKey(this);
  }

  restore() {
    const mini = this.mini;
    if (!mini) return;
    this.mini = null;
    const from = { x: mini.offsetLeft, y: this.screen.h - 64, w: 64, h: 64 };
    mini.remove();
    // re-pack remaining miniwindows
    this.screen.tiles.querySelectorAll<HTMLElement>(".nx-miniwindow").forEach((m, i) => (m.style.left = 64 * (i + 1) + "px"));
    this.visible = true;
    this.el.style.display = "";
    this.animate(from, () => {}, true);
    this.screen.makeKey(this);
  }

  /** 120 ms scale animation between the window rect and a tile rect (§9.8). */
  private animate(rect: { x: number; y: number; w: number; h: number }, done: () => void, reverse = false) {
    const s = this.el.style;
    if (!animations) { done(); return; }
    const t = `translate(${rect.x - this.x}px, ${rect.y - this.y}px) scale(${rect.w / this.w}, ${rect.h / this.h})`;
    s.transformOrigin = "0 0";
    s.transition = "none";
    s.transform = reverse ? t : "";
    void this.el.offsetWidth; // flush
    s.transition = "transform 120ms linear";
    s.transform = reverse ? "" : t;
    setTimeout(() => { s.transition = "none"; s.transform = ""; done(); }, 130);
  }
}

/** Convert a mouse event to Screen coordinates. */
export const screenPoint = (e: MouseEvent) => ({ x: e.clientX / scale, y: e.clientY / scale });
