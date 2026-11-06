// Phase 1 (tablet): records the two device takes for the showcase film with Playwright
// device emulation (default: Galaxy Tab S4, configurable with MOBILE_DEVICE) driving the
// SAME web app — the Android app in android/ is native Kotlin, which Playwright cannot
// automate; EffectCraft frames these recordings into a tablet mockup (scenes 4 and 7).
//
//   m04-atvetel   van checked in, "camera" photos shot/uploaded   (0:35-0:50)
//   m07-kiadas    the customer's digital signature, handover      (1:30-1:45)
//
// Notes:
//  - "Taking photos" is simulated with a file chooser upload of the real van photos from
//    VAN_PHOTOS_ROOT (the intake flow lets the office upload shots of the van).
//  - The digital signature is drawn on a canvas element if the page has one; without the
//    tablet-only inspection UI running, this step logs and skips rather than scripting
//    something that was never on screen.
import fs from 'node:fs';

import { config, logger, vanPhotos, vanPhotoPath } from '../config.js';
import { hero, type Hero } from '../record-data.js';
import { take, bestEffort } from './capture.js';

const orderUrl = (h: Hero): string => (h.orderId ? `/hu/orders/${h.orderId}` : '/hu/orders');

/** The four real van photos to "shoot" during the intake, when they exist on disk. */
function photosToShoot(): string[] {
  const keys: (keyof typeof vanPhotos)[] = ['sprinter', 'loadSpace', 'sprinterSide'];
  const files = keys.map((k) => vanPhotoPath(k)).filter((f) => fs.existsSync(f));
  if (!files.length) logger.warn('no real van photos found for the intake upload — the photo step will be skipped');
  return files;
}

// 0:35-0:50 — átvétel: the van is checked in, its photos land on the job.
async function m04Atvetel(h: Hero): Promise<void> {
  await take('m04-atvetel', { start: orderUrl(h), device: process.env.MOBILE_DEVICE ?? 'Galaxy Tab S4', seed: 404 }, async (rec) => {
    await rec.wait(1600);
    await bestEffort(rec, 'order header beat', () => rec.beat('order-header', rec.page.locator('h1').first(), 3200));

    // The intake slip: scroll to it, beat, then the photos.
    await bestEffort(rec, 'intake slip', () => rec.beat('intake-slip', { card: 'Átvétel-átadás' }, 2600));
    await rec.scroll(420, 900);
    await bestEffort(rec, 'open the intake section', () =>
      rec.click(rec.page.locator('button:has-text("Átvétel"), summary:has-text("Átvétel"), [aria-label*="Átvétel"]').first()),
    );

    // "Shooting" the checklist photos: upload the real van shots through the file input.
    const files = photosToShoot();
    if (files.length) {
      const input = rec.page.locator('input[type=file]').first();
      await bestEffort(rec, 'upload intake photos', () => rec.upload(input, files));
      await rec.wait(1400);
      await bestEffort(rec, 'photo grid beat', () => rec.beat('photos', rec.page.locator('img[src*="/api/"], img[src^="blob:"], [class*=photo], img').first(), 3000));
    }
    await rec.scroll(300, 800);
  });
}

// 1:30-1:45 — kiadás: the customer signs on the screen, the van is handed over.
async function m07Kiadas(h: Hero): Promise<void> {
  await take('m07-kiadas', { start: orderUrl(h), device: process.env.MOBILE_DEVICE ?? 'Galaxy Tab S4', seed: 707 }, async (rec) => {
    await rec.wait(1600);
    await bestEffort(rec, 'stage rail beat', () => rec.beat('stage-rail', rec.page.locator('main').first(), 2800));

    // Handover inspection section, then the signature.
    await rec.scroll(480, 900);
    await bestEffort(rec, 'open handover section', () =>
      rec.click(rec.page.locator('button:has-text("Kiadás"), summary:has-text("Kiadás"), [aria-label*="Kiadás"]').first()),
    );
    await rec.wait(1000);

    const canvas = rec.page.locator('canvas').first();
    if ((await canvas.count()) > 0) {
      // The customer's sign: two confident strokes across the pad.
      await rec.sign(canvas, [
        [[0.08, 0.72], [0.16, 0.25], [0.26, 0.55], [0.34, 0.2], [0.44, 0.66], [0.5, 0.55]],
        [[0.2, 0.8], [0.62, 0.78], [0.88, 0.55]],
      ]);
      await rec.beat('signature', canvas, 2600);
      await bestEffort(rec, 'confirm handover', () => rec.click(rec.page.locator('button:has-text("Aláír"), button:has-text("Kiadás"), button[type="submit"]').first()));
    } else {
      logger.warn('no <canvas> on the page — the tablet-only signature pad step was skipped');
    }

    await rec.wait(1200);
    await bestEffort(rec, 'stage ready beat', () => rec.beat('ready', rec.page.locator('text=/Kész|Átvehető/').first(), 2400));
  });
}

export async function recordMobile(): Promise<void> {
  const device = process.env.MOBILE_DEVICE ?? 'Galaxy Tab S4';
  logger.info({ url: config.mobileUrl, device }, 'recording the tablet takes');
  const h = await hero();
  await m04Atvetel(h);
  await m07Kiadas(h);
}

if (process.argv[1]?.replace(/\\/g, '/').endsWith('record-mobile.ts')) {
  recordMobile().catch((err) => {
    logger.error({ err: (err as Error).message }, 'tablet recording failed');
    process.exitCode = 1;
  });
}
