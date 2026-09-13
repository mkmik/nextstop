import { el } from "../chrome/ui";
import { Screen, trackDrag } from "./screen";

export type DropHandler = (paths: string[], e: MouseEvent) => void;
const targets = new Map<HTMLElement, DropHandler>();

/** Register a drop target for file drags. Handlers receive absolute paths. */
export function dropTarget(elm: HTMLElement, h: DropHandler) {
  elm.dataset.drop = "1";
  targets.set(elm, h);
}

export let dragging: string[] | null = null;

/**
 * Start dragging `paths` (§7.8): a 48×48 icon (+N badge) follows the cursor at 50 % opacity.
 * On release the innermost registered target under the cursor gets the paths.
 */
export function startDrag(screen: Screen, e: MouseEvent, paths: string[], iconUrl: string, opts: { dispatch?: boolean; onEnd?: (target: HTMLElement | null, e: MouseEvent) => void } = {}) {
  const img = el("img"); img.src = iconUrl;
  const ghost = el("div", "drag-ghost", img);
  if (paths.length > 1) ghost.append(el("div", "drag-badge", `+${paths.length - 1}`));
  screen.drag.append(ghost);
  dragging = paths;
  const place = (ev: MouseEvent) => { ghost.style.left = ev.clientX / screen.scaleOf() - 24 + "px"; ghost.style.top = ev.clientY / screen.scaleOf() - 24 + "px"; };
  place(e);
  trackDrag(e, {
    threshold: 0,
    move: (_x, _y, ev) => place(ev),
    up: (ev) => {
      ghost.remove();
      dragging = null;
      const t = (document.elementFromPoint(ev.clientX, ev.clientY) as HTMLElement | null)?.closest<HTMLElement>("[data-drop]") ?? null;
      if (t && opts.dispatch !== false) targets.get(t)?.(paths, ev);
      opts.onEnd?.(t, ev);
    },
  });
}
