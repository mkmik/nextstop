import "./styles/palette.css";
import "./styles/chrome.css";
import { Screen } from "./screen/screen";
import { el } from "./chrome/ui";
import { NXWindow } from "./chrome/window";
import { Scroller } from "./chrome/scroller";

const screen = new Screen();
const demo = new URLSearchParams(location.search).get("demo");

if (demo === "chrome") demoChrome();
if (demo === "windows") demoWindows();

function demoChrome() {
  const box = el("div", "demo");
  box.append(
    el("div", "demo-row", el("div", "nx-raised nx-button", "Raised Button"), el("div", "nx-sunken nx-well", "Sunken well")),
    el("div", "demo-row", el("span", "", "Regular 12 px"), el("span", "nx-bold", "Bold 12 px"), el("span", "nx-small", "Small 10 px"), el("span", "nx-mono", "Mono 11 px")),
  );
  screen.el.append(box);
}

function demoWindows() {
  const a = new NXWindow(screen, { title: "Untitled", x: 60, y: 60, w: 320, h: 240, minW: 160, minH: 100 });
  const b = new NXWindow(screen, { title: "Scroller Demo", x: 300, y: 200, w: 300, h: 260, minW: 160, minH: 100 });
  const list = el("div", "demo-list");
  for (let i = 1; i <= 100; i++) list.append(el("div", "demo-line", `Line ${i}`));
  const view = el("div", "demo-view", list);
  const sc = new Scroller(view, true);
  const row = el("div", "demo-scrollrow", view, sc.el);
  const hview = el("div", "demo-hview", el("div", "demo-hcontent", "This line is much wider than the window so the horizontal scroller gets a knob. ".repeat(3)));
  const hsc = new Scroller(hview, false);
  b.content.append(row, hview, hsc.el);
  b.content.style.display = "flex"; b.content.style.flexDirection = "column";
  a.show(); b.show();
}
