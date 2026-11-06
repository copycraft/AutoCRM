// Track file -> motion for one take: where the virtual camera looks and how close, where
// the cursor is and whether it shows, and where clicks land. Rules in film/motion.md.
//
//   node film/motion/camera.mjs test-dashboard
//
// Writes film/effectcraft/<take>.motion.json, which film/effectcraft/build-take.js reads.
// Coordinates there are comp pixels with the browser window at rest (camera at 1.0x).

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const FILM = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const name = process.argv[2];
const track = JSON.parse(fs.readFileSync(path.join(FILM, 'capture', 'tracks', `${name}.track.json`), 'utf8'));

// The comp and the browser window in it, at rest.
const COMP = { w: 1920, h: 1080 };
const C = [COMP.w / 2, COMP.h / 2];
const WINDOW_W = 1612; // 84 % of the frame: leaves the top margin for the section label
const BAR = 34;
const k = WINDOW_W / track.view.width; // CSS px -> comp px
const screenH = track.view.height * k;
const win = { x: (COMP.w - WINDOW_W) / 2, y: (COMP.h - screenH - BAR) / 2, w: WINDOW_W, h: screenH + BAR };
const screen = { x: win.x, y: win.y + BAR, w: WINDOW_W, h: screenH };
const rest = ([x, y]) => [screen.x + x * k, screen.y + y * k];

// Camera rules.
const ZOOM = [1, 2];
const FILL = 0.55; // a beat's rectangle fills this much of the frame
const LEAD = 0.3; // s the camera starts before a beat
const DEAD_ZONE = 0.6; // the cursor may roam this middle share of the frame
const OMEGA = 9; // spring stiffness: critically damped, settles in about 0.5 s
const MARGIN = 30; // how far past the window's edge the camera may show
const BRIDGE = 0.8; // s: a shorter gap between beats keeps the camera close instead of dipping out
const IDLE_HIDE = 1.5; // s without movement or clicks before the cursor fades
const FADE_OUT = 0.3;
const FADE_IN = 0.15;

const fps = track.fps;
const frames = Math.round(track.duration * fps);

const beats = [...track.beats].sort((a, b) => a.t - b.t).map((b) => ({ ...b }));
for (let i = 0; i + 1 < beats.length; i++) {
  if (beats[i + 1].t - LEAD - beats[i].until < BRIDGE) beats[i].until = beats[i + 1].t - LEAD;
}

function cursorAt(t) {
  const c = track.cursor;
  if (t <= c[0].t) return [c[0].x, c[0].y];
  for (let i = 1; i < c.length; i++) {
    if (c[i].t >= t) {
      const u = (t - c[i - 1].t) / (c[i].t - c[i - 1].t || 1);
      return [c[i - 1].x + (c[i].x - c[i - 1].x) * u, c[i - 1].y + (c[i].y - c[i - 1].y) * u];
    }
  }
  return [c.at(-1).x, c.at(-1).y];
}

// The last time the hand did something: moved, or pressed the button.
function lastActiveBefore(t) {
  let last = 0;
  const c = track.cursor;
  for (let i = 1; i < c.length && c[i].t <= t; i++) if (c[i].x !== c[i - 1].x || c[i].y !== c[i - 1].y) last = c[i].t;
  for (const e of track.events) if ((e.type === 'down' || e.type === 'up') && e.t <= t) last = Math.max(last, e.t);
  return last;
}

// Where the camera wants to be at time t: [focus x, focus y, zoom], before smoothing.
function wanted(t) {
  const beat = beats.find((b) => t >= b.t - LEAD && t <= b.until);
  if (!beat) return [C[0], C[1], 1];
  const [x, y, w, h] = beat.rect;
  const zoom = Math.min(ZOOM[1], Math.max(ZOOM[0], Math.min((COMP.w * FILL) / (w * k), (COMP.h * FILL) / (h * k))));
  let [fx, fy] = rest([x + w / 2, y + h / 2]);

  // Keep the cursor inside the dead zone of the frame this would show.
  const [cx, cy] = rest(cursorAt(t));
  const hw = (COMP.w / zoom / 2) * DEAD_ZONE;
  const hh = (COMP.h / zoom / 2) * DEAD_ZONE;
  fx = Math.min(Math.max(fx, cx - hw), cx + hw);
  fy = Math.min(Math.max(fy, cy - hh), cy + hh);
  return [fx, fy, zoom];
}

// Never show past the window's edge (plus a margin); when the frame is wider than that,
// stay centred on it.
function clamp([fx, fy, zoom]) {
  const bounds = [win.x - MARGIN, win.y - MARGIN, win.x + win.w + MARGIN, win.y + win.h + MARGIN];
  const hw = COMP.w / zoom / 2;
  const hh = COMP.h / zoom / 2;
  const axis = (f, lo, hi, half) => (hi - lo <= 2 * half ? (lo + hi) / 2 : Math.min(Math.max(f, lo + half), hi - half));
  return [axis(fx, bounds[0], bounds[2], hw), axis(fy, bounds[1], bounds[3], hh), zoom];
}

// A critically damped spring per channel, stepped at 240 Hz.
const state = clamp(wanted(0));
const velocity = [0, 0, 0];
const cam = [];
const SUB = 8;
for (let i = 0; i < frames; i++) {
  for (let s = 0; s < SUB; s++) {
    const t = (i + s / SUB) / fps;
    const target = clamp(wanted(t));
    const dt = 1 / fps / SUB;
    for (let c = 0; c < 3; c++) {
      velocity[c] += (OMEGA * OMEGA * (target[c] - state[c]) - 2 * OMEGA * velocity[c]) * dt;
      state[c] += velocity[c] * dt;
    }
  }
  const [fx, fy, zoom] = clamp(state);
  // The camera null's anchor is the comp centre: a rest point p lands at P + zoom * (p - C).
  cam.push({ pos: [C[0] - zoom * (fx - C[0]), C[1] - zoom * (fy - C[1])].map((v) => +v.toFixed(2)), scale: +(zoom * 100).toFixed(3) });
}

// The cursor shows while the hand is active, fades after IDLE_HIDE, and fades back in
// (looking ahead, so it is visible when a move starts rather than popping in mid-way).
const cursor = [];
let opacity = 100;
for (let i = 0; i < frames; i++) {
  const t = i / fps;
  const soon = Math.min(track.duration, t + FADE_IN);
  const active = t - lastActiveBefore(t) < IDLE_HIDE || lastActiveBefore(soon) > t;
  opacity = active ? Math.min(100, opacity + 100 / (FADE_IN * fps)) : Math.max(0, opacity - 100 / (FADE_OUT * fps));
  cursor.push({ pos: rest(cursorAt(t)).map((v) => +v.toFixed(2)), opacity: Math.round(opacity) });
}

const clicks = track.events.filter((e) => e.type === 'down').map((e) => ({ t: e.t, pos: rest([e.x, e.y]).map((v) => +v.toFixed(2)) }));
const keys = track.events.filter((e) => e.type === 'key').map((e) => ({ t: e.t, key: e.key }));

const out = {
  take: name, fps, duration: frames / fps, sequence: track.sequence,
  layout: { comp: COMP, window: win, screen, bar: BAR, footageScale: +((WINDOW_W / (track.view.width * track.scale)) * 100).toFixed(4) },
  cam, cursor, clicks, keys,
};
const file = path.join(FILM, 'effectcraft', `${name}.motion.json`);
fs.writeFileSync(file, JSON.stringify(out));
const peak = Math.max(...cam.map((c) => c.scale));
console.log(`${name}: ${frames} frames, zoom up to ${(peak / 100).toFixed(2)}x, ${clicks.length} click(s), ${keys.length} key(s) -> ${path.relative(FILM, file)}`);
