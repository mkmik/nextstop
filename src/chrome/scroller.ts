import { el, triUp, triDown, triLeft, triRight } from "./ui";
import { trackDrag, pressable } from "../screen/screen";

const LINE = 18;

/**
 * NeXT scroller (§7.10): sunken track, raised knob with a dimple, both arrow buttons at the
 * bottom (vertical) / right (horizontal). Drives an `overflow: hidden` element.
 */
export class Scroller {
  el: HTMLElement;
  private track: HTMLElement;
  private knob: HTMLElement;
  private arrows: HTMLElement[];
  private ro: ResizeObserver;

  constructor(public target: HTMLElement, public vertical = true) {
    this.knob = el("div", "nx-raised knob", el("div", "nx-sunken dimple"));
    this.track = el("div", "track", this.knob);
    const a1 = el("div", "nx-raised arrow", vertical ? triUp() : triLeft());
    const a2 = el("div", "nx-raised arrow", vertical ? triDown() : triRight());
    this.arrows = [a1, a2];
    this.el = el("div", "nx-sunken nx-scroller " + (vertical ? "v" : "h"), this.track, a1, a2);
    pressable(a1, () => this.scrollBy(-LINE));
    pressable(a2, () => this.scrollBy(LINE));
    // arrow buttons stay pressed and auto-repeat is out of scope (single step per click)
    this.track.addEventListener("mousedown", (e) => {
      if (e.button !== 0 || e.target === this.knob || this.knob.contains(e.target as Node)) return;
      e.stopPropagation();
      const r = this.knob.getBoundingClientRect();
      const before = vertical ? e.clientY < r.top : e.clientX < r.left;
      this.scrollBy((before ? -1 : 1) * this.visible);
    });
    this.knob.addEventListener("mousedown", (e) => {
      if (e.button !== 0) return;
      e.stopPropagation();
      const start = this.pos;
      trackDrag(e, { threshold: 0, move: (dx, dy) => {
        const free = this.trackLen - this.knobLen;
        if (free <= 0) return;
        this.pos = start + ((vertical ? dy : dx) / free) * (this.total - this.visible);
      } });
    });
    target.classList.add("nx-scroll");
    target.addEventListener("scroll", () => this.sync());
    target.addEventListener("wheel", (e) => {
      e.preventDefault();
      if (vertical) target.scrollTop += e.deltaY; else target.scrollLeft += e.deltaX || e.deltaY;
    }, { passive: false });
    this.ro = new ResizeObserver(() => this.sync());
    this.ro.observe(target);
    this.sync();
  }

  get total() { return this.vertical ? this.target.scrollHeight : this.target.scrollWidth; }
  get visible() { return this.vertical ? this.target.clientHeight : this.target.clientWidth; }
  get pos() { return this.vertical ? this.target.scrollTop : this.target.scrollLeft; }
  set pos(v: number) { if (this.vertical) this.target.scrollTop = v; else this.target.scrollLeft = v; }
  get trackLen() { return this.vertical ? this.track.clientHeight : this.track.clientWidth; }
  private knobLen = 0;

  scrollBy(d: number) { this.pos = this.pos + d; }

  /** Recompute the knob from the target's metrics. Call after content changes. */
  sync() {
    const total = this.total, vis = this.visible, tl = this.trackLen;
    const fits = total <= vis + 0.5;
    this.arrows.forEach((a) => a.classList.toggle("disabled", fits));
    this.knob.style.display = fits ? "none" : "";
    if (fits) return;
    this.knobLen = Math.max(16, Math.round((tl * vis) / total));
    const off = Math.round(((tl - this.knobLen) * this.pos) / (total - vis));
    const s = this.knob.style;
    if (this.vertical) { s.top = off + "px"; s.height = this.knobLen + "px"; }
    else { s.left = off + "px"; s.width = this.knobLen + "px"; }
  }

  /** True when the content overflows the target. */
  get overflowing() { return this.total > this.visible + 0.5; }

  destroy() { this.ro.disconnect(); }
}
