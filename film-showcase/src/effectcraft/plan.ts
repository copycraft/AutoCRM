// The 2-minute film's structure: 8 fixed scene slots across 120 seconds (the storyboard),
// each pointing at its recorded take (or a photo-only scene), with the compositing rules
// EffectCraft's build-showcase.js reads. The times are storyboard-frozen; the takes are
// stretched/trimmed onto their slots by the compositor, per scene:
//   1  Nyitás            0:00-0:10   photo + title overlay (no recording)
//   2  Érdeklődés        0:10-0:20   web take
//   3  Ajánlat           0:20-0:35   split: web quote | email approval graphic
//   4  Átvétel           0:35-0:50   tablet mockup, photo "flash" on the shots
//   5  Gyártás           0:50-1:15   web kanban (+ HR side scene inside the take)
//   6  Számlázás         1:15-1:30   web take
//   7  Kiadás            1:30-1:45   tablet mockup, signature
//   8  Reports & Outro   1:45-2:00   web chart, then logo + contact fade-in
import fs from 'node:fs';
import path from 'node:path';

import { config, logger, vanPhotos, vanPhotoPath } from '../config.js';

export interface ScenePlan {
  n: number;
  name: string;
  /** The section label as it appears on screen (`03 · AJÁNLAT & MEGRENDELÉS`). */
  label: string;
  start: number;
  end: number;
  /** Flow of this scene's compositing: single web take, split letter, tablet, pure photo. */
  kind: 'photo' | 'web' | 'split' | 'tablet' | 'outro';
  /** The recorded take folder under videos/web or videos/mobile (without the device dir). */
  take?: string;
  track?: string;
  /** Scene 3 right half: the drawn email-approval graphic with these lines. */
  emailLines?: string[];
  /** Sample moments (seconds, take-relative) when a "photo flash" fires (scene 4). */
  flashAt?: number[];
  /** Title for the whole film (scene 1) / end card (scene 8). */
  title?: string;
  subtitle?: string;
}

export interface FilmPlan {
  fps: number;
  width: number;
  height: number;
  duration: number;
  projectName: string;
  title: string;
  subtitle: string;
  photos: Record<string, string>;
  audio: string | null;
  contact: { line1: string; line2: string; line3: string };
  scenes: ScenePlan[];
}

/** The storyboard, with takes wired in. `takes` root is videos/web or videos/mobile. */
export function storyboard(available: Map<string, string>): ScenePlan[] {
  const scene = (
    n: number,
    name: string,
    label: string,
    start: number,
    end: number,
    opts: Partial<ScenePlan>,
  ): ScenePlan => {
    const base: ScenePlan = { n, name, label, start, end, kind: 'web', ...opts };
    const takeRoot = available.get(base.take ?? '');
    if (takeRoot) {
      const rel = takeRoot.replaceAll('\\', '/');
      base.take = rel;
      base.track = `${rel}/${base.take.split('/').pop()}.track.json`;
    } else if (base.take) {
      logger.warn({ take: base.take }, 'take missing — the scene will show its slate until the take is recorded');
      base.take = undefined;
    }
    if (!base.take && base.kind !== 'photo' && base.kind !== 'outro') {
      base.kind = base.kind === 'tablet' ? 'photo' : 'web';
    }
    return base;
  };

  return [
    scene(1, 'Nyitás', '01 · NYITÁS', 0, 10, {
      kind: 'photo',
      subtitle: 'Hűtős átalakítás · MNP-421',
    }),
    scene(2, 'Érdeklődés', '02 · ÉRDEKLŐDÉS', 10, 20, { take: 'web/s02-erdeklodes' }),
    scene(3, 'Ajánlat & Megrendelés', '03 · AJÁNLAT & MEGRENDELÉS', 20, 35, {
      take: 'web/s03-ajanlat-megrendeles',
      kind: 'split',
      emailLines: ['Tárgy: Megrendelés – hűtős felépítmény', 'Szép napot! Elfogadjuk az ajánlatot,', 'a Sprinter(MNP-421) átvételére hétfőn', 'várjuk a műhelyt. Szabó Gergely'],
    }),
    scene(4, 'Átvétel', '04 · ÁTVÉTEL', 35, 50, { kind: 'tablet', take: 'mobile/m04-atvetel' }),
    scene(5, 'Gyártás', '05 · GYÁRTÁS', 50, 75, { take: 'web/s05-gyartas' }),
    scene(6, 'Számlázás', '06 · SZÁMLÁZÁS', 75, 90, { take: 'web/s06-szamlazas' }),
    scene(7, 'Kiadás', '07 · KIADÁS', 90, 105, { kind: 'tablet', take: 'mobile/m07-kiadas' }),
    scene(8, 'Jelentések és zárás', '08 · JELENTÉSEK', 105, 120, { take: 'web/s08-reports', kind: 'outro' }),
  ];
}

/** Photos "shot" in scene 4 flash when the track log shows an upload: spaced clicks. */
function flashMoments(trackFile: string | undefined, kind: ScenePlan['kind']): number[] {
  if (!trackFile || kind !== 'tablet' || !fs.existsSync(trackFile)) return [];
  try {
    const track = JSON.parse(fs.readFileSync(trackFile, 'utf8')) as { events?: { t: number; type: string }[] };
    const downs = (track.events ?? []).filter((e) => e.type === 'down');
    // A flash at each of the first three spaced interactions after 6 s (the upload happens late).
    return downs.filter((e) => e.t > 6).slice(0, 3).map((e) => +e.t.toFixed(2));
  } catch {
    return [];
  }
}

export function buildPlan(): FilmPlan {
  // Collected takes: <name> -> absolute path of its folder (videos/web/<name> etc).
  const available = new Map<string, string>();
  for (const dir of [config.videos.web, config.videos.mobile]) {
    if (!fs.existsSync(dir)) continue;
    for (const name of fs.readdirSync(dir)) {
      const p = path.join(dir, name);
      if (fs.statSync(p).isDirectory() && fs.existsSync(path.join(p, 'seq'))) {
        available.set(path.relative(config.root, p).replaceAll('\\', '/'), p);
      }
    }
  }

  const scenes = storyboard(available);
  for (const s of scenes) {
    if (s.flashAt === undefined) s.flashAt = flashMoments(s.track, s.kind);
  }

  const photos: Record<string, string> = {};
  for (const key of Object.keys(vanPhotos) as (keyof typeof vanPhotos)[]) {
    photos[key] = vanPhotoPath(key).replaceAll('\\', '/');
  }

  const plan: FilmPlan = {
    fps: config.captureFps,
    width: 1920,
    height: 1080,
    duration: config.filmDuration,
    projectName: config.effectcraft.projectName,
    title: 'Autotherm: A Teljes Folyamat',
    subtitle: 'Hűtős átalakítás · MNP-421 · Szeged',
    photos,
    audio: config.audioBedPath || null,
    contact: {
      line1: 'Autotherm',
      line2: 'Járműátalakítás · Szeged',
      line3: 'autotherm.hu · +36 62 555 0100',
    },
    scenes,
  };
  return plan;
}
