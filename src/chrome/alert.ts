import { el, svg } from "./ui";
import { Screen, pressable } from "../screen/screen";
import { NXWindow } from "./window";
import { icon } from "../icons";

let open = 0;

/** Modal NeXT alert panel (§7.11). Resolves with the index of the pressed button. Enter = default, Escape = leftmost. */
export function alert(screen: Screen, o: { message: string; detail?: string; buttons?: string[]; def?: number }): Promise<number> {
  const buttons = o.buttons ?? ["OK"];
  const def = o.def ?? buttons.length - 1;
  return new Promise((resolve) => {
    const win = new NXWindow(screen, {
      title: "ReWorkspace", x: Math.round((screen.w - 380) / 2), y: Math.round((screen.h - 140) / 2), w: 380, h: 140,
      resizable: false, miniaturizable: false, closable: false, layer: screen.modal,
    });
    const finish = (i: number) => {
      document.removeEventListener("keydown", onKey, true);
      win.destroy();
      if (--open === 0) screen.modal.classList.remove("active");
      resolve(i);
    };
    const onKey = (e: KeyboardEvent) => {
      e.stopPropagation(); e.preventDefault();
      if (e.key === "Enter") finish(def);
      else if (e.key === "Escape") finish(0);
    };
    const btns = buttons.map((label, i) => {
      const b = el("div", "nx-raised nx-button alert-btn" + (i === def ? " default" : ""), el("span", "", label));
      if (i === def) b.append(svg(10, 8, `<path d="M9 0.5V5.5H2M4 3L1.5 5.5L4 8" fill="none" stroke="#000"/>`, "ret"));
      pressable(b, () => finish(i));
      return b;
    });
    const img = el("img", "alert-icon"); img.src = icon["alert"];
    win.content.append(img,
      el("div", "alert-text", el("div", "nx-bold", o.message), el("div", "", o.detail ?? "")),
      el("div", "alert-buttons", ...btns));
    document.addEventListener("keydown", onKey, true);
    open++;
    screen.modal.classList.add("active");
    win.show();
  });
}
