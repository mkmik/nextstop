// ponytail: browser-only stand-in for the Rust backend so demo pages and headless screenshots work. Not shipped logic.
import type { Entry } from "./backend";

const HOME = "/Users/marko";
const tree: Record<string, string[]> = {
  "/": ["Applications/", "Library/", "System/", "Users/", "bin/", "etc/", "tmp/", ".hidden/", "README.txt"],
  "/Applications": ["Safari.app*", "TextEdit.app*", "Utilities/"],
  "/Applications/Utilities": ["Terminal.app*", "Console.app*"],
  "/Users": ["Shared/", "marko/"],
  [HOME]: ["Desktop/", "Documents/", "Downloads/", "Library/", "Movies/", "Music/", "Pictures/", "Public/", ".zshrc", "notes.txt", "photo.png", "project.zip", "song.mp3", "talk.mov", "main.rs"],
  [HOME + "/Documents"]: ["Reports/", "Taxes/", "budget.csv", "letter.txt", "plan.md", "thesis.pdf", "a very long file name that needs truncating.txt"],
  [HOME + "/Documents/Reports"]: ["q1.txt", "q2.txt", "q3.txt", "q4.txt", "summary.md", ...Array.from({ length: 40 }, (_, i) => `report-${String(i + 1).padStart(2, "0")}.txt`)],
  [HOME + "/Documents/Reports/summary.md"]: [],
  [HOME + "/Documents/Taxes"]: ["2024/", "2025/"],
  [HOME + "/Documents/Taxes/2025"]: ["return.pdf", "receipts/"],
  [HOME + "/Documents/Taxes/2025/receipts"]: ["jan.png", "feb.png"],
  [HOME + "/Desktop"]: ["todo.txt", "link -> Documents@"],
};
for (const k of Object.keys(tree)) for (const n of tree[k]) { const clean = n.replace(/[/*@]$/, "").replace(/ -> .*/, ""); const p = k === "/" ? "/" + clean : k + "/" + clean; if (n.endsWith("/") && !tree[p]) tree[p] = []; }

function list(path: string, showHidden: boolean): Entry[] {
  const names = tree[path];
  if (!names) throw new Error(`${path}: No such file or directory`);
  const es = names.map((n) => {
    const is_symlink = n.endsWith("@"), is_app = n.endsWith("*"), is_dir = n.endsWith("/");
    const name = n.replace(/[/*@]$/, "").replace(/ -> .*/, "");
    return { name, path: path === "/" ? "/" + name : path + "/" + name, is_dir: is_dir || is_symlink, is_symlink, is_app, size: is_dir ? 0 : 1234 * (name.length + 1), modified: 1_750_000_000 + name.length * 86400, hidden: name.startsWith(".") };
  }).filter((e) => showHidden || !e.hidden);
  es.sort((a, b) => +b.is_dir - +a.is_dir || a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
  return es;
}

export async function mock<T>(cmd: string, a: Record<string, unknown>): Promise<T> {
  const r = (v: unknown) => v as T;
  switch (cmd) {
    case "log": return r(undefined);
    case "platform": return r(`${navigator.platform} (browser)`);
    case "home_dir": return r(HOME);
    case "root_dirs": return r(["/"]);
    case "filter_existing": return r(a.paths);
    case "list_dir": return r(list(a.path as string, a.showHidden as boolean));
    case "file_meta": return r({ created: 1_700_000_000, modified: 1_750_000_000, size: 4321, mode: "-rw-r--r--", owner: "marko", is_dir: false });
    case "read_text_head": return r(Array.from({ length: 30 }, (_, i) => `line ${i + 1}: the quick brown fox jumps over the lazy dog`).join("\n"));
    case "dir_size": return r(123456789);
    case "trash_is_empty": return r(true);
    case "default_dock": return r(["/Applications/Safari.app", "/Applications/TextEdit.app"]);
    case "load_state": return r({ state: null, no_anim: false });
    case "save_state": case "open_path": case "open_with": case "launch_app": case "trash_paths": case "destroy_paths": case "move_paths": case "copy_paths": case "new_folder": case "duplicate_paths": case "empty_trash": case "quit":
      console.log("mock", cmd, JSON.stringify(a)); return r(cmd === "new_folder" ? a.parent + "/New Folder" : cmd === "duplicate_paths" ? [] : undefined);
    case "trash_list": return r([]);
    case "app_icon_png": throw new Error("mock: no icons");
    default: throw new Error(`mock: no command ${cmd}`);
  }
}
