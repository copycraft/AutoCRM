// Phase 1 (web): records the desktop takes for the 2-minute showcase film at 1920x1080,
// Hungarian on screen, STR out of a live dev server (see .env.example: WEB_APP_URL).
//
// The film's storyboard, one take per scene (never a long unbroken recording):
//   s02-erdeklodes            new lead parsed                                  (0:10-0:20)
//   s03-ajanlat-megrendeles   quote built, client says yes                     (0:20-0:35)
//   s05-gyartas               Kanban moves + HR side-scene                     (0:50-1:15)
//   s06-szamlazas             the auto invoice                                 (1:15-1:30)
//   s08-reports               revenue charts (outro speeds on in EffectCraft)  (1:45-2:00)
// Scene 1 (opening photo + title) and the end card are composed in EffectCraft only.
//
// Login selectors realism notes: steps that may not exist on a given seed are wrapped in
// bestEffort() — the take still records the pages it can reach, and the log says which
// beats to re-shoot once the film stack is seeded (film/capture/backend-film.ps1 +
// seed-film.mjs).
import { logger, config } from '../config.js';
import { hero, type Hero } from '../record-data.js';
import { take, bestEffort } from './capture.js';

const leadUrl = (h: Hero): string => (h.leadId ? `/hu/leads/${h.leadId}` : '/hu/leads');
const orderUrl = (h: Hero): string => (h.orderId ? `/hu/orders/${h.orderId}` : '/hu/orders');

// 0:10-0:20 — the enquiry lands: the bell, the new lead with its Weboldal source tag.
async function s02Erdeklodes(h: Hero): Promise<void> {
  await take('s02-erdeklodes', { start: '/hu', seed: 21 }, async (rec) => {
    await rec.wait(600);
    await bestEffort(rec, 'bell beat', () => rec.beat('bell', rec.page.locator('a[href*="/notifications"]').first(), 2600));
    await bestEffort(rec, 'open notifications', () => rec.click(rec.page.locator('a[href*="/notifications"]').first()));
    await rec.wait(1200);

    await rec.navigate('/hu/leads');
    await rec.wait(800);
    await bestEffort(rec, 'open the hero lead', () =>
      rec.click(rec.page.locator('table tbody tr a').first()),
    );
    await rec.wait(1800);
    if (h.leadId) await bestEffort(rec, 'lead detail beat', () => rec.beat('lead-detail', rec.page.locator('h1').first(), 3000));
  });
}

// 0:20-0:35 — quote built on the lead, then the client's yes becomes a megrendelés.
async function s03Ajanlat(h: Hero): Promise<void> {
  await take('s03-ajanlat-megrendeles', { start: leadUrl(h), seed: 33 }, async (rec) => {
    await rec.wait(1200);
    // Stage chip: Új → Árajánlat, quoted value typed.
    await bestEffort(rec, 'stage chip beat', () =>
      rec.beat('lead-stage', rec.page.locator('text=/Új|Megkeresve|Árajánlat/').first(), 1600),
    );
    await bestEffort(rec, 'move to a quote stage', () => rec.click(rec.page.locator('button:has-text("Árajánlat"), [role="button"]:has-text("Árajánlat")').first()));
    await bestEffort(rec, 'quote value beat', () =>
      rec.beat('quote-value', rec.page.locator('input#lf-quote, text=/Ft/').first(), 2400),
    );
    await rec.wait(1500);

    // The client's yes: conversion to an order in a dialog.
    await bestEffort(rec, 'convert to order', () =>
      rec.click(rec.page.locator('button:has-text("Megrendelé"), a:has-text("Megrendelé")').first()),
    );
    await rec.wait(1300);
    await bestEffort(rec, 'conversion dialog beat', () => rec.beat('conversion', rec.page.locator('dialog, [role="dialog"]').first(), 2600));
    await rec.navigate(orderUrl(h));
    await rec.wait(1200);
    await bestEffort(rec, 'build spec beat', () => rec.beat('build-spec', rec.page.locator('main').first(), 3200));
    await rec.scroll(260, 800);
  });
}

// 0:50-1:15 — production: the pickup board's cards move, then the HR side-scene.
async function s05Gyartas(): Promise<void> {
  await take('s05-gyartas', { start: '/board', seed: 55 }, async (rec) => {
    await rec.wait(1400);
    await bestEffort(rec, 'board beat', () => rec.beat('pickup-board', rec.page.locator('main').first(), 2400));

    // Move the first card one column to the right (the drag is the "task moving").
    const card = rec.page.locator('[data-testid*=card], [class*=card], article').filter({ hasText: /MNP|Hópehely|Sprinter/ }).first();
    await bestEffort(rec, 'kanban drag', () => rec.drag(card, 340, 0));
    await rec.wait(1400);
    const card2 = rec.page.locator('[data-testid*=card], [class*=card], article').filter({ hasText: /MNP|Hópehely|Sprinter/ }).first();
    await bestEffort(rec, 'kanban drag 2', () => rec.drag(card2, 340, 0));
    await rec.wait(1200);

    // HR side-scene: the team, a flagged document.
    await rec.navigate('/hu/hr');
    await rec.wait(1200);
    await bestEffort(rec, 'hr directory beat', () => rec.beat('hr-directory', rec.page.locator('main').first(), 2800));
    await rec.scroll(300, 900);
  });
}

// 1:15-1:30 — the invoice from the order, and the invoice list.
async function s06Szamlazas(h: Hero): Promise<void> {
  await take('s06-szamlazas', { start: orderUrl(h), seed: 66 }, async (rec) => {
    await rec.wait(1200);
    await bestEffort(rec, 'open the invoice dialog', () =>
      rec.click(rec.page.locator('button:has-text("Számla"), a:has-text("Számla")').first()),
    );
    await rec.wait(1400);
    await bestEffort(rec, 'invoice preview beat', () => rec.beat('invoice', rec.page.locator('dialog, [role="dialog"], main').first(), 3000));
    await bestEffort(rec, 'issue it', () => rec.click(rec.page.locator('button:has-text("Számláz"), button:has-text("Készítés"), button[type="submit"]').first()));
    await rec.wait(1600);

    await rec.navigate('/hu/invoices');
    await rec.wait(1200);
    await bestEffort(rec, 'invoice list beat', () => rec.beat('invoice-list', rec.page.locator('main').first(), 2800));
  });
}

// 1:45-2:00 — reports: revenue chart, fleet heat map; the outro composes on top in AE.
async function s08Reports(): Promise<void> {
  await take('s08-reports', { start: '/hu/reports', seed: 88 }, async (rec) => {
    await rec.wait(1400);
    await bestEffort(rec, 'revenue chart beat', () => rec.beat('revenue-chart', rec.page.locator('main').first(), 3600));
    await rec.scroll(340, 900);
    await rec.wait(1200);
    await rec.navigate('/hu');
    await rec.wait(1400);
    await bestEffort(rec, 'dashboard tiles beat', () => rec.beat('dashboard', rec.page.locator('main').first(), 3200));
  });
}

export async function recordWeb(): Promise<void> {
  logger.info({ url: config.webUrl }, 'recording the web takes (1920x1080 headless)');
  const h = await hero();
  await s02Erdeklodes(h);
  await s03Ajanlat(h);
  await s05Gyartas();
  await s06Szamlazas(h);
  await s08Reports();
}

if (process.argv[1]?.replace(/\\/g, '/').endsWith('record-web.ts')) {
  recordWeb().catch((err) => {
    logger.error({ err: (err as Error).message }, 'web recording failed');
    process.exitCode = 1;
  });
}
