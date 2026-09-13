// Path helpers that work for "/" (macOS) and "C:\" (Windows) roots.
let win = false;
export const setWindowsPaths = (on: boolean) => (win = on);
export const sep = () => (win ? "\\" : "/");
const strip = (p: string) => (p.length > 1 && /[\\/]$/.test(p) && !/^[A-Za-z]:\\$/.test(p) ? p.slice(0, -1) : p);
export const isRoot = (p: string) => p === "/" || /^[A-Za-z]:\\$/.test(p);

export function basename(p: string) {
  if (isRoot(p)) return p;
  const t = strip(p);
  return t.slice(t.lastIndexOf(sep()) + 1);
}
export function dirname(p: string): string | null {
  if (isRoot(p)) return null;
  const t = strip(p);
  const i = t.lastIndexOf(sep());
  return i <= 0 ? (win ? t.slice(0, 3) : "/") : win && i === 2 ? t.slice(0, 3) : t.slice(0, i);
}
export const join = (dir: string, name: string) => (isRoot(dir) ? dir + name : dir + sep() + name);

/** Directories listed by each Browser column to reach `p`: on Windows the first is `null` (synthetic "Computer"). */
export function ancestors(p: string): (string | null)[] {
  const out: (string | null)[] = [];
  let cur: string | null = p;
  while (cur) { out.unshift(cur); cur = dirname(cur); }
  if (win) out.unshift(null);
  return out;
}
