import { invoke } from "@tauri-apps/api/core";
import { mock } from "./mock";

export const hasTauri = "__TAURI_INTERNALS__" in window;

/** Call a Rust command. Outside Tauri (demo pages, headless screenshots) a small in-browser mock answers. */
export function call<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  return hasTauri ? invoke<T>(cmd, args) : mock<T>(cmd, args);
}

export interface Entry { name: string; path: string; is_dir: boolean; is_symlink: boolean; is_app: boolean; size: number; modified: number; hidden: boolean }
export interface Meta { created: number | null; modified: number; size: number; mode: string; owner: string | null; is_dir: boolean }
