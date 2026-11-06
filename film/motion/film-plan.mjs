// film/script.md -> film/effectcraft/film.json: every scene's time span, section label,
// shot list, caption and narration, for film/effectcraft/build-film.js.
//
//   node film/motion/film-plan.mjs

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const FILM = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const lines = fs.readFileSync(path.join(FILM, 'script.md'), 'utf8').split(/\r?\n/);
const seconds = (mmss) => mmss.split(':').reduce((m, s) => m * 60 + Number(s), 0);
const plain = (md) => md.replace(/`([^`]*)`/g, '$1').replace(/\*+([^*]+)\*+/g, '$1').trim();

const scenes = [];
let scene = null;
for (const line of lines) {
  const heading = line.match(/^## (\d+) · (.+) \((\d+:\d+)–(\d+:\d+)\)$/);
  if (heading) {
    scene = { n: Number(heading[1]), name: heading[2], start: seconds(heading[3]), end: seconds(heading[4]), label: null, shots: [], caption: null, narration: null };
    scenes.push(scene);
    continue;
  }
  if (!scene) continue;
  const row = line.match(/^\| (\d+:\d+) \| (.+) \| (.+) \|$/);
  if (row) {
    const label = row[2].match(/`(\d\d · [^`]+)`/);
    if (label && !scene.label) scene.label = label[1];
    const picture = plain(row[2].replace(/^Section label `[^`]+`\.\s*/, '').replace(/^`\d\d · [^`]+`\.\s*/, ''));
    scene.shots.push({ t: +(seconds(row[1]) - scene.start).toFixed(2), picture, source: plain(row[3]) });
  }
  const caption = line.match(/^\*\*Felirat[^*]*:\*\* (.+)$/);
  if (caption) scene.caption = plain(caption[1]);
  if (line.startsWith('> ')) scene.narration = line.slice(2).trim();
}

const out = { fps: 30, duration: scenes.at(-1).end, scenes };
fs.writeFileSync(path.join(FILM, 'effectcraft', 'film.json'), JSON.stringify(out, null, 1));
for (const s of scenes) console.log(`${String(s.n).padStart(2)} ${s.start}-${s.end}s ${s.label ?? '(title)'} | ${s.shots.length} shots | ${s.caption}`);
