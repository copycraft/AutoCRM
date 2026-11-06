// Central configuration for the showcase film pipeline, read from .env (see .env.example).
// Every script imports this module first, so changing one file re-configures the pipeline.
import fs from 'node:fs';
import path from 'node:path';
import { pino } from 'pino';

import './dotenv.js';

const ROOT = path.resolve(import.meta.dirname, '..'); // src/ -> film-showcase/
const REPO = path.resolve(ROOT, '..');

function env(name: string, fallback: string): string {
  const v = process.env[name];
  return v === undefined || v === '' ? fallback : v;
}

/** Resolve a possibly relative path against the film-showcase root. */
function abs(p: string): string {
  return path.isAbsolute(p) ? p : path.resolve(ROOT, p);
}

export const config = {
  root: ROOT,
  repo: REPO,

  webUrl: env('WEB_APP_URL', 'http://localhost:3000'),
  mobileUrl: env('MOBILE_APP_URL', env('WEB_APP_URL', 'http://localhost:3000')),
  apiUrl: env('BACKEND_URL', 'http://127.0.0.1:8080'),

  filmDuration: Number(env('FILM_DURATION', '120')),
  captureScale: Number(env('CAPTURE_SCALE', '2')), // 2 = 3840x2160 for a sharp 2x zoom
  captureFps: Number(env('CAPTURE_FPS', '30')),

  accountsFile: abs(env('FILM_ACCOUNTS_FILE', '../film/data/accounts.txt')),
  vanPhotosRoot: env('VAN_PHOTOS_ROOT', 'C:/Users/vasta/Documents/PROJECTS/autotherm-photos'),

  effectcraft: {
    projectName: env('EFFECTCRAFT_PROJECT_NAME', 'showcase-film'),
    // Optional head-less runner; empty means: write the script, the human runs it once
    // inside EffectCraft (the same way film/effectcraft/build-film.js is run today).
    cli: env('EFFECTCRAFT_CLI', ''),
    exportPath: abs(env('EFFECTCRAFT_EXPORT_PATH', 'output/showcase-master.mp4')),
  },

  audioBedPath: env('AUDIO_BED_PATH', ''),

  videos: {
    web: abs('videos/web'),
    mobile: abs('videos/mobile'),
  },
  assets: abs('assets'),
  output: abs('output'),
  effectcraftDir: abs('effectcraft'),
};

/** The film's admin login, from the film accounts file (email / password on the `admin` line). */
export function adminLogin(): { email: string; password: string } {
  const candidates = [
    config.accountsFile,
    path.resolve(config.repo, 'backend', 'local-test-accounts.txt'),
  ];
  for (const file of candidates) {
    if (!fs.existsSync(file)) continue;
    const line = fs.readFileSync(file, 'utf8').split('\n').find((l) => l.startsWith('admin'));
    const m = line?.match(/(\S+@\S+)\s+\/\s+(\S+)/);
    if (m) return { email: m[1]!, password: m[2]! };
  }
  throw new Error(`No admin account found in ${candidates.join(' or ')} — start the film stack first?`);
}

/** The real van photos for the opening and the outro (verified at startup). */
export const vanPhotos = {
  sprinter: { file: 'IMG_20260511_092828.jpg', folder: 'car-2' },
  hearse: { file: 'IMG_20260818_082645.jpg', folder: 'car-1' },
  loadSpace: { file: 'IMG_20260511_092940.jpg', folder: 'car-2' },
  sprinterSide: { file: 'IMG_20260511_092854.jpg', folder: 'car-2' },
};

export function vanPhotoPath(key: keyof typeof vanPhotos): string {
  const p = vanPhotos[key];
  return path.join(config.vanPhotosRoot, p.folder, p.file);
}

export function checkVanPhotos(): string[] {
  const missing: string[] = [];
  for (const key of Object.keys(vanPhotos) as (keyof typeof vanPhotos)[]) {
    const file = vanPhotoPath(key);
    if (!fs.existsSync(file)) missing.push(file);
  }
  return missing;
}

export const logger = pino(
  {
    level: env('LOG_LEVEL', 'info'),
    transport: { target: 'pino-pretty', options: { colorize: true } },
  },
);
