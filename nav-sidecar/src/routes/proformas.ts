import { Router } from 'express';
import type { Request, Response } from 'express';
import type { Config } from '../config.js';
import { badRequest, notFound } from '../errors.js';
import { ProformaCache, renderProforma } from '../proformas.js';
import { proformaSchema } from '../schemas.js';

/**
 * Proformas (díjbekérő) — no NAV involvement.
 *
 * This router never touches the NAV client: it takes no credentials, opens no
 * transaction, and produces nothing NAV will ever see. It is mounted alongside
 * `/invoices` only because it renders from the same data.
 */
export function proformaRoutes(config: Config): { router: Router; cache: ProformaCache } {
  const cache = new ProformaCache(config.proformaTtlMs, config.proformaCacheMax);
  const router = Router();

  /** Build and render a proforma. Nothing is reported. */
  router.post('/', async (request: Request, response: Response) => {
    const payload = proformaSchema.parse(request.body);
    response.status(201).json(await renderProforma(config, cache, payload));
  });

  /**
   * The rendered proforma, by id.
   *
   * Served from the in-memory cache the render left behind. There is no store:
   * once the entry expires the answer is a 404 saying so, and the caller — who
   * holds the payload — renders it again.
   */
  router.get('/:id/pdf', (request: Request, response: Response) => {
    const id = request.params.id;
    if (typeof id !== 'string' || id.length === 0) {
      throw badRequest('the path must name a proforma id');
    }

    const entry = cache.get(id);
    if (!entry) {
      throw notFound(
        `no rendered proforma ${id} is held. The sidecar keeps proformas in memory only ` +
          `(for ${Math.round(config.proformaTtlMs / 1000)}s); POST /proformas again to re-render it.`,
      );
    }

    response.setHeader('Content-Type', 'application/pdf');
    response.setHeader(
      'Content-Disposition',
      `inline; filename="${safeFilename(entry.invoiceNumber)}.pdf"`,
    );
    response.send(entry.pdf);
  });

  return { router, cache };
}

function safeFilename(value: string): string {
  return value.replace(/[^A-Za-z0-9._-]/g, '_').slice(0, 80) || 'proforma';
}
