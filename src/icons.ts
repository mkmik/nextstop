// Icon registry: every SVG in assets/icons, keyed by file stem.
const files = import.meta.glob("./assets/icons/*.svg", { eager: true, query: "?url", import: "default" }) as Record<string, string>;
export const icon: Record<string, string> = {};
for (const [p, url] of Object.entries(files)) icon[p.replace(/.*\/(.*)\.svg$/, "$1")] = url;

const byExt: Record<string, string> = {};
const map: [string, string][] = [
  ["txt md log csv json yaml yml toml ini", "file-text"],
  ["png jpg jpeg gif bmp webp svg icns ico", "file-image"],
  ["zip tar gz tgz bz2 xz 7z rar dmg", "file-archive"],
  ["c h cpp go rs py js ts sh zig rb java swift m", "file-code"],
  ["pdf", "file-pdf"],
  ["mp3 wav flac aac m4a ogg", "file-audio"],
  ["mp4 mov mkv avi webm", "file-video"],
  ["app exe lnk", "application"],
];
for (const [exts, name] of map) for (const e of exts.split(" ")) byExt[e] = name;

export const ext = (name: string) => name.includes(".") ? name.slice(name.lastIndexOf(".") + 1).toLowerCase() : "";
export const fileIconName = (name: string) => byExt[ext(name)] ?? "file-generic";
/** 48×48 icon URL for a Browser entry. */
export const iconFor = (name: string, isDir: boolean, isApp = false) => icon[isDir ? "folder" : isApp ? "application" : fileIconName(name)];
/** 16×16 icon URL for a Browser cell (only some types have a small variant). */
export function smallIconFor(name: string, isDir: boolean, isApp = false) {
  const n = isDir ? "folder" : isApp ? "application" : fileIconName(name);
  return icon[n + "-16"] ?? icon["file-generic-16"];
}
/** Human kind for the Inspector. */
export function kindFor(name: string, isDir: boolean, isApp = false) {
  if (isDir) return "Folder";
  if (isApp) return "Application";
  const e = ext(name);
  const k = fileIconName(name);
  const words: Record<string, string> = { "file-text": "text document", "file-image": "image", "file-archive": "archive", "file-code": "source code", "file-pdf": "document", "file-audio": "audio", "file-video": "video" };
  return e ? `${e.toUpperCase()} ${words[k] ?? "file"}` : "Document";
}
