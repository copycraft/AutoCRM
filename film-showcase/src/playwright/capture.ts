// The capture engine, shared by record-web.ts and record-mobile.ts.
//
// Follows the conventions proven in film/capture/lib.mjs (see film/motion.md):
//  - Chrome's own CDP screencast at 2x device scale (Playwright's recordVideo webm is
//    VP8 at 1 Mbit/s, too soft to zoom into in the final film), rendered into a steady
//    30 fps JPEG sequence that EffectCraft imports directly;
//  - the cursor is driven like a hand (arced, eased moves with a small overshoot);
//  - a track file (<take>.track.json) logs cursor positions, clicks, typing, scrolls
//    and beats, from which the virtual camera and ripples are built.
//
// A take is `take(name, { device? }, async (rec) => { await rec.click(...); ... })`.
import fs from 'node:fs';
import path from 'node:path';

import { chromium, devices as playwrightDevices, type Browser, type BrowserContext, type CDPSession, type Locator, type Page } from 'playwright';

import { config, logger, adminLogin } from '../config.js';

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Beat {
  t: number;
  until: number;
  name: string;
  rect: Rect;
}

export interface TrackEvent {
  t: number;
  type: 'down' | 'up' | 'type' | 'key' | 'scroll' | 'navigate';
  x?: number;
  y?: number;
  text?: string;
  until?: number;
  dy?: number;
  key?: string;
  url?: string;
}

export interface Track {
  fps: number;
  scale: number;
  view: { width: number; height: number };
  device: { name: string; viewport: { width: number; height: number } } | null;
  duration: number;
  cursor: { t: number; x: number; y: number }[];
  events: TrackEvent[];
  beats: Beat[];
  /** Path of the first frame of the rendered JPEG sequence, relative to the project root. */
  sequence?: string;
}

export interface TakeOptions {
  /** Playwright device to emulate; undefined = standard desktop. */
  device?: string;
  /** The URL to sign in on and start from after login. */
  start?: string;
  /** Repeatable randomness seed, so a re-recorded take moves the same way. */
  seed?: number;
  /** Skip the app sign-in (the capture smoke test records a synthetic page). */
  skipLogin?: boolean;
}

export interface Rec {
  page: Page;
  /** Plain sleep. */
  wait(ms: number): Promise<void>;
  move(target: Locator | [number, number], opts?: { ms?: number }): Promise<void>;
  click(target: Locator | [number, number]): Promise<void>;
  type(text: string): Promise<void>;
  fill(target: Locator, value: string): Promise<void>;
  select(target: Locator, label: string): Promise<void>;
  key(key: string): Promise<void>;
  scroll(dy: number, ms?: number): Promise<void>;
  navigate(url: string): Promise<void>;
  /** Upload files through a file chooser (simulated camera photos). */
  upload(input: Locator, files: string[]): Promise<void>;
  /** Draw a free sign on a canvas element (the digital signature pads): an array of
   *  strokes, each an array of [x, y] points in canvas-local fraction coordinates. */
  sign(canvas: Locator, strokes: [number, number][][]): Promise<void>;
  /** Press, move like a hand, release (Kanban cards, yard moves). */
  drag(target: Locator | [number, number], dx: number, dy: number): Promise<void>;
  /** From now for ms, the narration is about this element / titled section. */
  beat(name: string, target: Locator | { card: string }, ms: number): Promise<void>;
  rectOf(target: Locator | { card: string }): Promise<Rect>;
}

const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));
const easeInOut = (u: number) => (u < 0.5 ? 4 * u * u * u : 1 - (-2 * u + 2) ** 3 / 2);

export async function take(name: string, opts: TakeOptions, script: (rec: Rec) => Promise<void>): Promise<void> {
  const scale = config.captureScale;
  const out = opts.device ? config.videos.mobile : config.videos.web;
  const raw = path.join(out, name);
  fs.rmSync(raw, { recursive: true, force: true });
  fs.mkdirSync(raw, { recursive: true });

  const base = opts.device ? config.mobileUrl : config.webUrl;
  const localeOpts = { locale: 'hu-HU', timezoneId: 'Europe/Budapest', colorScheme: 'light' } as const;

  let browser: Browser | null = null;
  let context: BrowserContext | null = null;
  const started = Date.now();

  try {
    browser = await chromium.launch({
      headless: true,
      args: [`--force-device-scale-factor=${scale}`, '--disable-features=CalculateNativeWinOcclusion'],
    });

    interface DeviceSpec extends Record<string, unknown> {
      viewport: { width: number; height: number };
      deviceScaleFactor: number;
    }
    const devices = playwrightDevices as unknown as Record<string, DeviceSpec>;
    const spec = opts.device ? devices[opts.device] : undefined;
    if (opts.device && !spec) throw new Error(`Unknown Playwright device "${opts.device}"`);

    context = await browser.newContext({
      viewport: spec?.viewport ?? { width: 1920, height: 1080 },
      deviceScaleFactor: spec?.deviceScaleFactor ?? scale,
      ...(spec as Record<string, unknown> | undefined),
      ...localeOpts,
    });

    const page = await context.newPage();
    page.setDefaultTimeout(12_000);
    page.setDefaultNavigationTimeout(20_000);

    // Sign in (the film's admin line; the app itself is locale-hu at /hu/login).
    if (!opts.skipLogin) {
      const { email, password } = adminLogin();
      await page.goto(`${base}/hu/login`);
      await page.fill('#email', email);
      await page.fill('#password', password);
      await page.click('button[type="submit"]');
      await page.waitForURL((u: URL) => !u.pathname.endsWith('/login'));
    }
    const start = opts.start ?? '/hu';
    await page.goto(`${base}${start}`);
    await page.waitForLoadState('networkidle');
    // The Next dev-server badge is not part of the app.
    await page.addStyleTag({ content: 'nextjs-portal { display: none !important; }' });
    await sleep(800);

    // Frames arrive on every repaint, each with the browser's own timestamp (seconds).
    const frames: { t: number; file: string }[] = [];
    const writes: Promise<void>[] = [];
    const cdp: CDPSession = await context.newCDPSession(page);
    cdp.on('Page.screencastFrame', (frame) => {
      const { data, metadata, sessionId } = frame;
      void cdp
        .send('Page.screencastFrameAck', { sessionId })
        .catch(() => {});
      if (!metadata?.timestamp) return;
      const file = path.join(raw, `r_${String(frames.length).padStart(5, '0')}.jpg`);
      frames.push({ t: metadata.timestamp, file });
      writes.push(fs.promises.writeFile(file, Buffer.from(data, 'base64')));
    });

    // Repeatable randomness, so a re-recorded take moves the same way.
    let seed = opts.seed ?? 1;
    const random = (): number => {
      seed = (seed + 0x6d2b79f5) | 0;
      const t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
      return ((t ^ (t >>> 7)) | (t & 0x80000000 ? 0x1_0000_0000 : 0)) / 0x1_0000_0000;
    };

    const view = page.viewportSize() ?? { width: 1920, height: 1080 };
    const cursor = { x: Math.round(view.width * 0.78), y: Math.round(view.height * 0.74) };
    const track: Track = {
      fps: config.captureFps,
      scale,
      view: { width: view.width, height: view.height },
      device: opts.device ? { name: opts.device, viewport: { ...view } } : null,
      duration: 0,
      cursor: [],
      events: [],
      beats: [],
    };
    await page.mouse.move(cursor.x, cursor.y);

    await cdp.send('Page.startScreencast', {
      format: 'jpeg',
      quality: 92,
      maxWidth: view.width * scale,
      maxHeight: view.height * scale,
      everyNthFrame: 1,
    });
    const t0 = Date.now() / 1000;
    const now = (): number => +((Date.now() / 1000 - t0) % 86_400).toFixed(3);
    const log = (): void => {
      track.cursor.push({ t: now(), x: +cursor.x.toFixed(1), y: +cursor.y.toFixed(1) });
    };
    log();

    const rectOf = async (target: Locator | { card: string }): Promise<Rect> => {
      const loc = typeof target === 'string' || 'boundingBox' in target ? (target as Locator) : page.locator(`section[aria-label="${target.card}"]`);
      const box = (await loc.first().boundingBox()) ?? { x: 0, y: 0, width: view.width, height: view.height };
      const x = Math.max(0, box.x);
      const y = Math.max(0, box.y);
      const w = Math.min(view.width, box.x + box.width) - x;
      const h = Math.min(view.height, box.y + box.height) - y;
      return { x, y, w, h };
    };

    const coordsOf = async (target: Locator | [number, number]): Promise<[number, number]> => {
      if (Array.isArray(target)) return target;
      const r = await rectOf(target);
      return [r.x + r.w * (0.35 + random() * 0.3), r.y + r.h * (0.4 + random() * 0.2)];
    };

    const rec: Rec = {
      page,
      wait: (ms) => sleep(ms),

      async move(target, { ms } = {}) {
        const to = await coordsOf(target);
        const from = { ...cursor };
        const dx = to[0] - from.x;
        const dy = to[1] - from.y;
        const dist = Math.hypot(dx, dy) || 1;
        const dur = ms ?? Math.min(750, 260 + dist * 0.33);
        const bend = dist * (0.05 + random() * 0.06) * (random() < 0.5 ? -1 : 1);
        const ctrl: [number, number] = [from.x + dx / 2 - (dy / dist) * bend, from.y + dy / 2 + (dx / dist) * bend];
        const over = opts.device ? 1.5 + random() : 2 + random() * 2;
        const end: [number, number] = [to[0] + (dx / dist) * over, to[1] + (dy / dist) * over];
        const began = Date.now();
        for (;;) {
          const u = Math.min(1, (Date.now() - began) / dur);
          const e = easeInOut(u);
          cursor.x = (1 - e) ** 2 * from.x + 2 * (1 - e) * e * ctrl[0] + e * e * end[0];
          cursor.y = (1 - e) ** 2 * from.y + 2 * (1 - e) * e * ctrl[1] + e * e * end[1];
          await page.mouse.move(cursor.x, cursor.y);
          log();
          if (u >= 1) break;
          await sleep(12);
        }
        await sleep(40);
        cursor.x = to[0];
        cursor.y = to[1];
        await page.mouse.move(cursor.x, cursor.y);
        log();
      },

      async click(target) {
        if (target) await rec.move(target);
        await sleep(90 + random() * 80);
        track.events.push({ t: now(), type: 'down', x: +cursor.x.toFixed(1), y: +cursor.y.toFixed(1) });
        await page.mouse.down();
        await sleep(70 + random() * 40);
        await page.mouse.up();
        track.events.push({ t: now(), type: 'up' });
      },

      async type(text) {
        const event: TrackEvent = { t: now(), type: 'type', text };
        for (const ch of text) {
          await page.keyboard.type(ch);
          await sleep(60 + random() * 50);
        }
        event.until = now();
        track.events.push(event);
      },

      async fill(target, value) {
        await rec.click(target);
        await sleep(150);
        const isDate = (await target.first().getAttribute('type')) === 'date';
        await target.first().fill('');
        if (isDate) {
          await target.fill(value);
          track.events.push({ t: now(), type: 'type', text: value, until: now() });
        } else {
          await rec.type(value);
        }
      },

      async select(target, label) {
        await rec.click(target);
        await sleep(250);
        await target.selectOption({ label });
        await sleep(200);
      },

      async key(key) {
        track.events.push({ t: now(), type: 'key', key });
        await page.keyboard.press(key);
      },

      async scroll(dy, ms = 700) {
        track.events.push({ t: now(), type: 'scroll', dy });
        const steps = Math.max(6, Math.round(ms / 30));
        let done = 0;
        for (let i = 1; i <= steps; i++) {
          const to = Math.round(dy * easeInOut(i / steps));
          await page.mouse.wheel(0, to - done);
          done = to;
          await sleep(ms / steps);
        }
        await sleep(150);
      },

      async navigate(url) {
        track.events.push({ t: now(), type: 'navigate', url });
        await page.goto(`${base}${url}`);
        await page.waitForLoadState('networkidle').catch(() => {});
        await sleep(500);
      },

      async upload(input, files) {
        await input.setInputFiles(files.map((f) => path.resolve(f)));
        await sleep(600);
      },

      // A free sign on a canvas: each stroke is [[x1,y1],[x2,y2],...] in canvas-local
      // fraction coordinates (0..1). Drawn with moved mouse events, so the recording
      // shows the hand drawing.
      async sign(canvas, strokes) {
        await canvas.scrollIntoViewIfNeeded().catch(() => {});
        await sleep(300);
        const box = (await canvas.boundingBox()) ?? { x: 0, y: 0, width: 300, height: 120 };
        for (const stroke of strokes) {
          for (let i = 0; i < stroke.length; i++) {
            const x = box.x + (stroke[i]?.[0] ?? 0) * box.width;
            const y = box.y + (stroke[i]?.[1] ?? 0) * box.height;
            if (i === 0) await page.mouse.move(x, y);
            else await page.mouse.move(x, y, { steps: 4 });
            log();
            await sleep(24);
          }
          await sleep(120);
        }
        await sleep(400);
      },

      // Press on the element, move like a hand (eased), release: Kanban cards, yard moves.
      async drag(target, dx, dy) {
        const [sx, sy] = await coordsOf(target);
        cursor.x = sx;
        cursor.y = sy;
        await page.mouse.move(sx, sy);
        await rec.move([sx, sy], { ms: 250 });
        await sleep(120);
        await page.mouse.down();
        track.events.push({ t: now(), type: 'down', x: +sx.toFixed(1), y: +sy.toFixed(1) });
        await sleep(120);
        const steps = Math.max(8, Math.round(600 / 30));
        for (let i = 1; i <= steps; i++) {
          const e = easeInOut(i / steps);
          cursor.x = sx + dx * e;
          cursor.y = sy + dy * e;
          await page.mouse.move(cursor.x, cursor.y);
          log();
          await sleep(30);
        }
        await sleep(200);
        await page.mouse.up();
        track.events.push({ t: now(), type: 'up' });
        await sleep(500);
      },

      async beat(nm, target, ms) {
        const rect = await rectOf(target);
        track.beats.push({ t: now(), until: +(now() + ms / 1000).toFixed(3), name: nm, rect });
      },

      rectOf,
    };

    await script(rec);
    await sleep(400);
    track.duration = now();

    // A steady 30 fps sequence: every output frame is the latest repaint at its time.
    const t0Abs = frames[0]?.t ?? t0;
    let src = 0;
    const count = Math.round(track.duration * track.fps);
    const seq = path.join(raw, 'seq');
    fs.mkdirSync(seq, { recursive: true });
    for (let i = 0; i < count; i++) {
      const t = t0Abs + i / track.fps;
      while (src + 1 < frames.length && frames[src + 1]!.t <= t) src++;
      const f = frames[src]!.file;
      fs.copyFileSync(f, path.join(seq, `${name}_${String(i).padStart(5, '0')}.jpg`));
    }
    await Promise.allSettled(writes);

    track.sequence = path.relative(config.root, path.join(seq, `${name}_00000.jpg`)).replaceAll('\\', '/');
    fs.writeFileSync(path.join(raw, `${name}.track.json`), JSON.stringify(track, null, 1));

    const secs = ((Date.now() - started) / 1000).toFixed(1);
    logger.info({ take: name, recordedSecs: track.duration.toFixed(1), frames: frames.length, rendered: count, wallSecs: secs }, 'take recorded');
  } finally {
    await context?.close().catch(() => {});
    await browser?.close().catch(() => {});
  }
}

/** Hungarian-scoped first(): a locator inside the app that logs rather than explodes. */
export function bestEffort(rec: Rec, step: string, fn: () => Promise<void>): Promise<void> {
  return fn().catch((err: unknown) => {
    if (err instanceof Error && /TimeoutError|timeout/i.test(err.message)) {
      logger.warn({ take: step }, `step skipped (timeout): ${err.message.split('\n')[0]}`);
    } else {
      logger.warn({ err: (err as Error).message }, `step skipped: ${step}`);
    }
  });
}
