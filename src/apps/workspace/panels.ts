import { el } from "../../chrome/ui";
import { Screen } from "../../screen/screen";
import { NXWindow, WinOpts } from "../../chrome/window";
import { Scroller } from "../../chrome/scroller";
import { call } from "../../backend";
import { icon } from "../../icons";
import pkg from "../../../package.json";

// ---- Console (§7.13): last 500 lines, HH:MM:SS timestamps, mirrored to stderr via the `log` command.
const lines: string[] = [];
let consoleView: HTMLElement | null = null;
let consoleScroller: Scroller | null = null;

export function log(msg: string) {
  const t = new Date();
  const ts = [t.getHours(), t.getMinutes(), t.getSeconds()].map((n) => String(n).padStart(2, "0")).join(":");
  const line = `${ts} ${msg}`;
  lines.push(line);
  if (lines.length > 500) lines.shift();
  console.log(line);
  call("log", { line }).catch(() => {});
  if (consoleView) {
    consoleView.append(el("div", "", line));
    while (consoleView.childElementCount > 500) consoleView.firstElementChild!.remove();
    consoleScroller!.sync();
    consoleView.scrollTop = consoleView.scrollHeight;
  }
}

export function createConsole(screen: Screen, geo: Partial<WinOpts>) {
  const win = new NXWindow(screen, { title: "Console", x: 200, y: 500, w: 480, h: 240, minW: 240, minH: 100, icon: icon["miniwindow"], ...geo });
  consoleView = el("div", "console-view nx-mono", ...lines.map((l) => el("div", "", l)));
  consoleScroller = new Scroller(consoleView, true);
  win.content.append(el("div", "console-row", consoleView, consoleScroller.el));
  return win;
}

// ---- Info panel (§7.12)
export function createInfo(screen: Screen) {
  const win = new NXWindow(screen, { title: "Info", x: Math.round((screen.w - 300) / 2), y: 120, w: 300, h: 200, resizable: false, icon: icon["workspace"] });
  const logo = el("img", "info-logo"); logo.src = icon["workspace"];
  const plat = el("div", "", "…");
  call<string>("platform").then((p) => (plat.textContent = p), () => (plat.textContent = navigator.platform));
  win.content.append(el("div", "info-body", logo,
    el("div", "info-name", "ReWorkspace"),
    el("div", "", `Version ${pkg.version}`),
    el("div", "", "A NeXTSTEP-style workspace. Not affiliated with NeXT or Apple."),
    plat));
  return win;
}
