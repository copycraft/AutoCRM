// Looks up the records the film follows (hero lead, the MNP-421 order) from the running
// backend, so the takes navigate straight to real pages instead of guessing rows in lists.
//
// The hero lead is what film/capture/seed-film.mjs created (film/data/hero.json); if that
// was never run, the newest website-sourced lead is used. The order for the van scenes is
// looked up by the plate, falling back to the oldest open order (or /orders alone).
import fs from 'node:fs';
import path from 'node:path';

import { config, logger, adminLogin } from './config.js';

interface leadLike {
  id: number;
  title?: string;
  stage?: string;
  source?: string;
}

interface orderLike {
  id: number;
  slug?: string;
  title?: string;
  vehicle?: { plate?: string } | string;
  stage?: string;
  status?: string;
}

export interface Hero {
  leadId: number | null;
  orderId: number | null;
}

export async function fetchJson(url: string, init?: RequestInit): Promise<unknown> {
  const res = await fetch(url, {
    ...init,
    headers: { 'content-type': 'application/json', ...(init?.headers ?? {}) },
  });
  if (!res.ok) throw new Error(`${init?.method ?? 'GET'} ${url} -> ${res.status}`);
  const text = await res.text();
  return text ? (JSON.parse(text) as unknown) : null;
}

export async function hero(): Promise<Hero> {
  // Token for API lookups (the film's admin line).
  let token = '';
  try {
    const { email, password } = adminLogin();
    const login = (await fetchJson(`${config.apiUrl}/api/auth/login`, {
      method: 'POST',
      body: JSON.stringify({ email, password, client: 'mobile', device_label: 'film showcase' }),
    })) as { token?: string } | null;
    token = login?.token ?? '';
    if (!token) throw new Error('no token in login response');
  } catch (err) {
    logger.warn({ err: (err as Error).message }, 'backend login failed — record ids will fall back to UI navigation alone');
    return { leadId: null, orderId: null };
  }
  const auth = { authorization: `Bearer ${token}` };

  let leadId: number | null = null;
  try {
    // The seed's hero (film/data/hero.json) first...
    const heroFile = path.resolve(config.repo, 'film', 'data', 'hero.json');
    if (fs.existsSync(heroFile)) {
      const heroJson = JSON.parse(fs.readFileSync(heroFile, 'utf8')) as { lead?: number };
      if (heroJson.lead) {
        leadId = heroJson.lead;
        logger.info({ leadId }, 'hero lead from film/data/hero.json');
      }
    }
    // ...otherwise the newest lead the API knows.
    if (!leadId) {
      const list = (await fetchJson(`${config.apiUrl}/api/leads`, { headers: auth })) as { items?: leadLike[] };
      const items = Array.isArray(list?.items) ? list.items : [];
      leadId = items.length ? Math.max(...items.map((l) => l.id)) : null;
      logger.info({ leadId }, 'newest lead on the server');
    }
  } catch (err) {
    logger.warn({ err: (err as Error).message }, 'lead lookup failed');
  }

  let orderId: number | null = null;
  try {
    const list = (await fetchJson(`${config.apiUrl}/api/orders`, { headers: auth })) as { items?: orderLike[] };
    const items = Array.isArray(list?.items) ? list.items : [];
    // The hero order: the Sprinter with plate MNP-421 (its vehicle/plate may nest or sit flat).
    const hit = items.find((o) =>
      [
        (o as unknown as Record<string, unknown>)['plate'],
        o.vehicle && typeof o.vehicle === 'object' ? o.vehicle.plate : undefined,
        o.vehicle && typeof o.vehicle === 'string' ? o.vehicle : undefined,
        o.title,
        o.slug,
      ].some((v) => typeof v === 'string' && v.toUpperCase().includes('MNP')),
    );
    orderId = (hit ?? items[0])?.id ?? null;
    logger.info({ orderId, of: items.length }, 'order for the van scenes');
  } catch (err) {
    logger.warn({ err: (err as Error).message }, 'order lookup failed');
  }

  return { leadId, orderId };
}
