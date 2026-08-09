// Regression test for the pet's timers.
//
//   node test/pet-timers.test.js
//
// Every timer on `pet` (lastEventAt, nextBlinkAt, hopUntil, shakeUntil) is
// stamped with Date.now(). tick() must compare against that same clock. It
// used to take requestAnimationFrame's timestamp instead, which counts from
// page load — roughly 57 years adrift from a wall-clock stamp. Nothing threw
// and nothing looked obviously broken, but the pet could never blink and never
// fall asleep, and one error left it shaking forever.
//
// So this drives the real tick() from ui/index.html against a clock we control,
// replacing only the rAF driver and the browser demo loop. No dependencies, no
// DOM: the canvas is stubbed, and shake is observed through the ctx.translate
// call tick() makes rather than by looking at pixels.

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const UI = path.join(__dirname, "..", "ui", "index.html");

function load(htmlPath, clock, { reduceMotion = false } = {}) {
  const html = fs.readFileSync(htmlPath, "utf8");
  let code = html.slice(html.indexOf("<script>") + 8, html.lastIndexOf("</script>"));
  code = code.replace(/requestAnimationFrame\(tick\);/g, "");
  code = code.replace("setInterval(() => handleEvent(demo[i++ % demo.length]), 4000);", "void 0;");
  code += "\nglobalThis.__tick = tick; globalThis.__pet = pet;";

  const shakeX = [];
  const ctx = new Proxy(
    { translate: (x) => shakeX.push(x) },
    { get: (t, k) => (k in t ? t[k] : () => {}), set: () => true }
  );
  const node = () => ({
    className: "", children: [], style: {}, classList: { add() {}, toggle() {} },
    appendChild() {}, prepend() {}, removeChild() {}, remove() {},
    setAttribute() {}, addEventListener() {},
    getContext: () => ctx, width: 160, height: 176,
  });

  const sandbox = {
    console: { log() {}, error() {} },
    Math,
    setTimeout: () => 0,
    setInterval: () => 0,
    Date: class extends Date { static now() { return clock.now; } },
    requestAnimationFrame: () => 0,
    addEventListener: () => {},
    matchMedia: (q) => ({ matches: reduceMotion && q.includes("reduced-motion") }),
    document: { getElementById: node, createElement: node, body: node() },
  };
  sandbox.window = sandbox;
  vm.createContext(sandbox);
  vm.runInContext(code, sandbox);
  return { sandbox, pet: sandbox.__pet, tick: sandbox.__tick, shakeX };
}

const failures = [];
function check(name, actual, expected) {
  const ok = actual === expected;
  console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : ` — got ${actual}, want ${expected}`}`);
  if (!ok) failures.push(name);
}

// Wall clock and the rAF timestamp advance together. tick() is handed the rAF
// value, exactly as the browser hands it over, so a regression to the old
// behavior fails here rather than passing by accident.
const clock = { now: 1700000000000 };
let raf = 120;
const { sandbox, pet, tick, shakeX } = load(UI, clock);
const step = (ms) => { clock.now += ms; raf += ms; tick(raf); };

step(0);
check("starts idle", pet.mode, "idle");

step(4 * 60 * 1000);
check("still awake before the 5 minute threshold", pet.mode, "idle");

step(2 * 60 * 1000);
check("falls asleep after 5 minutes of silence", pet.mode, "sleeping");

sandbox.handleEvent({ kind: "task_start", title: "Go" });
step(0);
check("an event wakes it into working", pet.mode, "working");

sandbox.handleEvent({ kind: "error", title: "Boom" });
shakeX.length = 0;
for (let i = 0; i < 8; i++) step(100);            // inside the 900ms shake window
check("shakes on error", shakeX.some((x) => x !== 0), true);
shakeX.length = 0;
for (let i = 0; i < 20; i++) step(200);           // 1s-5s later, well past it
check("stops shaking after the window", shakeX.some((x) => x !== 0), false);

sandbox.handleEvent({ kind: "task_done", title: "Done" });
check("hop window is 700ms from now", pet.hopUntil - clock.now, 700);
check("task_done returns it to idle", pet.mode, "idle");

let blinked = false;
for (let i = 0; i < 60; i++) { step(500); if (clock.now < pet.blinkUntil) blinked = true; }
check("blinks while idle", blinked, true);

// The pet is always on top and cannot be scrolled away from, so a system
// request for less motion has to actually stop the movement — while the state
// itself (working, sleeping) still comes through in the sprite.
{
  const rmClock = { now: 1_700_000_000_000 };
  const rm = load(UI, rmClock, { reduceMotion: true });
  let rmRaf = 0;
  const rmStep = (ms) => { rmClock.now += ms; rmRaf += ms; rm.tick(rmRaf); };

  rm.sandbox.handleEvent({ kind: "error", title: "Boom" });
  rm.shakeX.length = 0;
  for (let i = 0; i < 8; i++) rmStep(100);        // inside the shake window
  check("reduced motion: does not shake on error", rm.shakeX.some((x) => x !== 0), false);

  rm.sandbox.handleEvent({ kind: "task_start", title: "Go" });
  rmStep(0);
  check("reduced motion: still reports working", rm.pet.mode, "working");

  rmStep(6 * 60 * 1000);
  check("reduced motion: still falls asleep", rm.pet.mode, "sleeping");
}

console.log(failures.length ? `\n${failures.length} failing` : "\nall passing");
process.exit(failures.length ? 1 : 0);
