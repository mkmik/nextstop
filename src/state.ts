// Persisted state (§11). Loaded/saved in M5; until then these are the in-memory defaults.
export interface WinGeo { x: number; y: number; w?: number; h?: number; open: boolean }
export interface State {
  version: 1;
  os_window: { w: number; h: number };
  scale: 1 | 2;
  show_hidden: boolean;
  animations: boolean;
  menu_pos: { x: number; y: number };
  torn_menus: { path: string[]; x: number; y: number }[];
  dock: string[];
  shelf: string[];
  windows: {
    file_viewer: WinGeo & { w: number; h: number; path: string };
    inspector: WinGeo;
    console: WinGeo & { w: number; h: number };
    recycler: WinGeo & { w: number; h: number };
  };
}

export const defaults = (home: string): State => ({
  version: 1,
  os_window: { w: 1120, h: 832 },
  scale: 1,
  show_hidden: false,
  animations: true,
  menu_pos: { x: 4, y: 4 },
  torn_menus: [],
  dock: [],
  shelf: [],
  windows: {
    file_viewer: { x: 120, y: 40, w: 640, h: 480, path: home, open: true },
    inspector: { x: 780, y: 40, open: false },
    console: { x: 200, y: 500, w: 480, h: 240, open: false },
    recycler: { x: 300, y: 300, w: 320, h: 240, open: false },
  },
});

let saver: (() => void) | null = null;
export const onDirty = (f: () => void) => (saver = f);
/** Call after any change to `state`; the save is debounced by the installed saver (M5). */
export const dirty = () => saver?.();
