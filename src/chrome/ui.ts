// Tiny DOM helpers shared by all chrome.
export function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls = "", ...kids: (Node | string)[]): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  e.append(...kids);
  return e;
}

export function svg(w: number, h: number, inner: string, cls = ""): SVGSVGElement {
  const t = document.createElement("template");
  t.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" class="${cls}" shape-rendering="crispEdges">${inner}</svg>`;
  return t.content.firstChild as SVGSVGElement;
}

/** Right-pointing triangle, `s` px tall (NeXT ▸). */
export const triRight = (s = 6, fill = "currentColor") => svg(s, s, `<path d="M0 0L${s} ${s / 2}L0 ${s}Z" fill="${fill}"/>`);
export const triUp = (s = 7) => svg(s, s, `<path d="M0 ${s}L${s / 2} 0L${s} ${s}Z" fill="currentColor"/>`);
export const triDown = (s = 7) => svg(s, s, `<path d="M0 0L${s} 0L${s / 2} ${s}Z" fill="currentColor"/>`);
export const triLeft = (s = 7) => svg(s, s, `<path d="M${s} 0L0 ${s / 2}L${s} ${s}Z" fill="currentColor"/>`);

/** Middle-ellipsis truncation for file names: verylongfilena…ame.txt */
export function truncMiddle(s: string, max: number): string {
  if (s.length <= max) return s;
  const keep = max - 1, head = Math.ceil(keep * 0.6), tail = keep - head;
  return s.slice(0, head) + "…" + s.slice(s.length - tail);
}

export const isMac = navigator.platform.startsWith("Mac");
export const cmdKey = (e: KeyboardEvent) => (isMac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey);
