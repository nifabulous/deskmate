// Regression test for clicking a message title to jump to its session.
//
//   node test/focus-session.test.js
//
// Only Claude Code messages carrying a session id are clickable: the deep link
// behind this resolves a Claude session id, which means nothing for an opencode
// run or a shell script. A title that is clickable must also drop its
// drag-region attribute, or pressing it starts a window drag instead.
//
// Runs the real ui/index.html against a stubbed DOM and a stubbed Tauri bridge
// that records invoke() calls, so the whole chain is exercised without the app.

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const UI = path.join(__dirname, "..", "ui", "index.html");

const created = [];
const invokes = [];
const byId = {};

function makeNode() {
  const node = {
    className: "", textContent: "", title: "", children: [], style: { setProperty() {} },
    _attrs: new Map(), _handlers: {},
    classList: {
      add(c) { node.className += (node.className ? " " : "") + c; },
      contains(c) { return node.className.split(" ").includes(c); },
      toggle(c, on) {
        if (on) { if (!node.classList.contains(c)) node.classList.add(c); }
        else node.className = node.className.split(" ").filter((x) => x !== c).join(" ");
        return !!on;
      },
    },
    setAttribute(k, v) { node._attrs.set(k, v); },
    getAttribute(k) { return node._attrs.has(k) ? node._attrs.get(k) : null; },
    hasAttribute(k) { return node._attrs.has(k); },
    addEventListener(ev, fn) { (node._handlers[ev] ||= []).push(fn); },
    click() { (node._handlers.click || []).forEach((fn) => fn({ stopPropagation() {} })); },
    press(key) {
      (node._handlers.keydown || []).forEach((fn) =>
        fn({ key, preventDefault() {}, stopPropagation() {} }));
    },
    appendChild(c) { node.children.push(c); c.parentNode = node; },
    prepend(c) { node.children.unshift(c); c.parentNode = node; },
    removeChild(c) { node.children = node.children.filter((x) => x !== c); },
    remove() { if (node.parentNode) node.parentNode.removeChild(node); },
    querySelector() { return null; },
    getContext: () => ctxStub,
    rect: { left: 0, top: 0, right: 84, bottom: 90, width: 84, height: 90 },
    getBoundingClientRect: () => node.rect,
    width: 84, height: 90, scrollHeight: 0, clientHeight: 0,
  };
  created.push(node);
  return node;
}

const ctxStub = new Proxy({}, { get: () => () => {}, set: () => true });

const html = fs.readFileSync(UI, "utf8");
let code = html.slice(html.indexOf("<script>") + 8, html.lastIndexOf("</script>"));
code = code.replace(/requestAnimationFrame\(tick\);/g, "");
code += "\nglobalThis.publishHitRegion = publishHitRegion;";

const sandbox = {
  console: { log() {}, error() {} },
  Math,
  setTimeout: () => 0,
  setInterval: () => 0,
  requestAnimationFrame: () => 0,
  addEventListener: () => {},
  localStorage: { getItem: () => null, setItem() {} },
  // Cache by id: the UI holds on to the node it looked up, so a fresh one per
  // call would leave the test poking at an element nothing renders into.
  document: { getElementById: (id) => (byId[id] ||= makeNode()), createElement: makeNode, body: makeNode() },
  __TAURI__: {
    core: {
      invoke(name, args) {
        invokes.push({ name, args });
        return Promise.resolve(null);
      },
    },
    event: { listen: () => Promise.resolve(() => {}) },
  },
};
sandbox.window = sandbox;
vm.createContext(sandbox);
vm.runInContext(code, sandbox);

const failures = [];
function check(name, actual, expected) {
  const ok = actual === expected;
  console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : ` — got ${JSON.stringify(actual)}, want ${JSON.stringify(expected)}`}`);
  if (!ok) failures.push(name);
}

const SESSION = "c247fe2e-aaa2-4084-98b1-ddc4acc461e0";
const titleFor = (text) => created.find((n) => n.textContent === text && n.className.includes("title"));

sandbox.handleEvent({ kind: "tool_use", source: "claude-code", session: SESSION, title: "Claude Code msg", detail: "x" });
sandbox.handleEvent({ kind: "tool_use", source: "opencode", session: "run-9c71", title: "opencode msg", detail: "x" });
sandbox.handleEvent({ kind: "tool_use", source: "claude-code", title: "No session", detail: "x" });

const claudeTitle = titleFor("Claude Code msg");
const opencodeTitle = titleFor("opencode msg");
const noSessionTitle = titleFor("No session");

check("a Claude Code message with a session is clickable", claudeTitle.classList.contains("linked"), true);
check("an opencode message is not clickable", opencodeTitle.classList.contains("linked"), false);
check("a Claude Code message with no session is not clickable", noSessionTitle.classList.contains("linked"), false);

// A clickable title must not also be a drag region, or the press becomes a drag.
check("clickable title is not a drag region", claudeTitle.hasAttribute("data-tauri-drag-region"), false);
check("non-clickable title stays a drag region", opencodeTitle.hasAttribute("data-tauri-drag-region"), true);

invokes.length = 0;
claudeTitle.click();
const call = invokes.find((c) => c.name === "focus_session");
check("clicking invokes focus_session", !!call, true);
check("and passes the session id through unchanged", call && call.args.session, SESSION);

invokes.length = 0;
opencodeTitle.click();
check("clicking a non-clickable title does nothing", invokes.length, 0);

// The title is a div, so without these it is a button only to a mouse user:
// nothing announces it, and Tab cannot reach it.
check("clickable title announces itself as a button", claudeTitle.getAttribute("role"), "button");
check("clickable title is reachable by Tab", claudeTitle.tabIndex, 0);
check("non-clickable title is not in the tab order", opencodeTitle.tabIndex, undefined);

for (const key of ["Enter", " "]) {
  invokes.length = 0;
  claudeTitle.press(key);
  check(`pressing ${key === " " ? "Space" : key} opens the session`,
    invokes.some((c) => c.name === "focus_session"), true);
}

invokes.length = 0;
claudeTitle.press("a");
check("other keys do nothing", invokes.length, 0);

// Both scrollbars are hidden, so the top fade is the only sign that older
// messages are still above the fold — and it must not dim the top message
// when nothing is actually hidden.
const logEl = byId.log;
logEl.scrollHeight = 411;
logEl.clientHeight = 150;
sandbox.handleEvent({ kind: "tool_use", source: "claude-code", title: "Overflowing", detail: "x" });
check("fades the top edge when messages are hidden above", logEl.classList.contains("clipped"), true);

logEl.scrollHeight = 90;
sandbox.handleEvent({ kind: "tool_use", source: "claude-code", title: "Fits", detail: "x" });
check("no fade when everything fits", logEl.classList.contains("clipped"), false);

// The window is click-through except over the reported hit region. #app spans
// the full window width whatever is inside it, so reporting its box blocked
// clicks in a wide band either side of the pet, with nothing drawn there.
// Real geometry in a 220x270 window:
byId.petwrap.rect = { left: 68, top: 174, right: 152, bottom: 264, width: 84, height: 90 };
byId.log.rect = { left: 6, top: 24, right: 214, bottom: 174, width: 208, height: 150 };
// #app deliberately absent: the fix stops consulting it, so it is never looked up.

const lastRegion = () => [...invokes].reverse().find((c) => c.name === "set_hit_region")?.args;

byId.log.hidden = true;
sandbox.publishHitRegion();
check("panel shut: hit region is just the pet, not the full width",
  JSON.stringify(lastRegion()), JSON.stringify({ x: 68, y: 174, w: 84, h: 90 }));

byId.log.hidden = false;
sandbox.publishHitRegion();
check("panel open: hit region covers panel and pet together",
  JSON.stringify(lastRegion()), JSON.stringify({ x: 6, y: 24, w: 208, h: 240 }));

console.log(failures.length ? `\n${failures.length} failing` : "\nall passing");
process.exit(failures.length ? 1 : 0);
