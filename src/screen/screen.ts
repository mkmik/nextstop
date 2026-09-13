import { el } from "../chrome/ui";

/** Global scale of the fake Screen (1 or 2). Mouse deltas must be divided by it. */
export let scale = 1;

export type DragHandlers = {
  move?: (dx: number, dy: number, e: MouseEvent) => void;
  up?: (e: MouseEvent, moved: boolean) => void;
  threshold?: number; // px, default 4 (§9.3)
};

/** Track a mouse drag from `e` until mouse-up. `move` fires only after the threshold is exceeded. */
export function trackDrag(e: MouseEvent, h: DragHandlers) {
  const x0 = e.clientX, y0 = e.clientY, th = h.threshold ?? 4;
  let moved = false;
  const onMove = (ev: MouseEvent) => {
    const dx = (ev.clientX - x0) / scale, dy = (ev.clientY - y0) / scale;
    if (!moved && Math.hypot(dx, dy) * scale < th) return;
    moved = true;
    h.move?.(dx, dy, ev);
  };
  const onUp = (ev: MouseEvent) => {
    window.removeEventListener("mousemove", onMove, true);
    window.removeEventListener("mouseup", onUp, true);
    h.up?.(ev, moved);
  };
  window.addEventListener("mousemove", onMove, true);
  window.addEventListener("mouseup", onUp, true);
  e.preventDefault();
}

/** Press-and-release model (§9.2): show pressed on mouse-down, act on mouse-up only if still over the control. */
export function pressable(elm: HTMLElement, onClick: (e: MouseEvent) => void, opts: { pressedClass?: string; drag?: (e: MouseEvent) => void } = {}) {
  const cls = opts.pressedClass ?? "pressed";
  elm.addEventListener("mousedown", (e) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    elm.classList.add(cls);
    let dragging = false;
    trackDrag(e, {
      move: (_dx, _dy, ev) => {
        if (opts.drag && !dragging) { dragging = true; elm.classList.remove(cls); opts.drag(ev); }
      },
      up: (ev, moved) => {
        elm.classList.remove(cls);
        if (dragging || (moved && opts.drag)) return;
        if (elm.contains(document.elementFromPoint(ev.clientX, ev.clientY))) onClick(ev);
      },
    });
  });
}

export interface KeyTarget { el: HTMLElement; setKey(k: boolean): void; visible: boolean; key: boolean; opts: { onKeyDown?: (e: KeyboardEvent) => boolean | void } }

export class Screen {
  wins: KeyTarget[] = [];
  keyWin: KeyTarget | null = null;
  private zTop = 1;

  el = document.getElementById("screen")!;
  windows = document.getElementById("layer-windows")!;
  tiles = document.getElementById("layer-tiles")!;
  menus = document.getElementById("layer-menus")!;
  modal = document.getElementById("layer-modal")!;
  drag = document.getElementById("layer-drag")!;
  w = 0; h = 0;
  onLayout: (() => void)[] = [];

  constructor() {
    this.layout();
    window.addEventListener("resize", () => this.layout());
    // §9.9: right-click does nothing anywhere (the main menu popup is wired in main.ts).
    document.addEventListener("contextmenu", (e) => e.preventDefault());
    // §9.10: no drag-and-drop of DOM nodes/images.
    document.addEventListener("dragstart", (e) => e.preventDefault());
  }

  setScale(s: number) { scale = s; this.layout(); }

  register(w: KeyTarget) { this.wins.push(w); }
  unregister(w: KeyTarget) { this.wins = this.wins.filter((x) => x !== w); if (this.keyWin === w) this.dropKey(w); }
  front(w: KeyTarget) { w.el.style.zIndex = String(++this.zTop); }
  makeKey(w: KeyTarget) {
    this.front(w);
    if (this.keyWin === w) return;
    this.keyWin?.setKey(false);
    this.keyWin = w;
    w.setKey(true);
  }
  /** Key window went away: give key to the topmost visible window. */
  dropKey(w: KeyTarget) {
    if (this.keyWin !== w) return;
    w.setKey(false);
    this.keyWin = null;
    const next = this.wins.filter((x) => x.visible && x !== w).sort((a, b) => +b.el.style.zIndex - +a.el.style.zIndex)[0];
    if (next) this.makeKey(next);
  }

  layout() {
    this.w = Math.round(innerWidth / scale);
    this.h = Math.round(innerHeight / scale);
    this.el.style.width = this.w + "px";
    this.el.style.height = this.h + "px";
    this.el.style.transform = scale === 1 ? "" : `scale(${scale})`;
    this.onLayout.forEach((f) => f());
  }
}

export { el };
