import { createHash, timingSafeEqual } from 'node:crypto';
import express from 'express';
import type { Express, NextFunction, Request, Response } from 'express';
import { errorHandler, notFound, unauthorized } from './errors.js';
import type { NavContext } from './nav.js';
import { invoiceRoutes } from './routes/invoices.js';
import { proformaRoutes } from './routes/proformas.js';

export interface HealthResponse {
  status: 'ok';
  /** True when talking to the bundled fake NAV. Nothing is reported for real. */
  mockMode: boolean;
  environment: 'test' | 'production';
  /** Identifies this reporter to NAV. Not a secret. */
  softwareId: string;
  uptimeSeconds: number;
}

/**
 * The HTTP surface.
 *
 * Two groups of routes: `/invoices`, which reports to NAV, and `/proformas`,
 * which does not and never will.
 */
export function createApp(context: NavContext): Express {
  const app = express();

  app.disable('x-powered-by');
  app.use(express.json({ limit: context.config.bodyLimit }));

  /**
   * Liveness only: no credentials are used and NAV is not contacted, so this
   * answers while the tax authority is down and can be polled as often as a
   * container runtime likes.
   */
  app.get('/health', (_request: Request, response: Response<HealthResponse>) => {
    response.json({
      status: 'ok',
      mockMode: context.config.mockMode,
      environment: context.config.environment,
      softwareId: context.config.software.softwareId,
      uptimeSeconds: Math.round(process.uptime()),
    });
  });

  // Everything past this point acts under the company's NAV credentials.
  const token = context.config.callerToken;
  if (token !== undefined) {
    const expected = digest(token);
    app.use((request: Request, _response: Response, next: NextFunction) => {
      const header = request.get('authorization') ?? '';
      const given = header.startsWith('Bearer ') ? header.slice('Bearer '.length).trim() : '';
      // Equal-length digests, compared in constant time.
      if (!timingSafeEqual(digest(given), expected)) throw unauthorized();
      next();
    });
  }

  app.use('/invoices', invoiceRoutes(context));
  app.use('/proformas', proformaRoutes(context.config).router);

  app.use((request: Request) => {
    throw notFound(`no route for ${request.method} ${request.path}`);
  });

  app.use(errorHandler);

  return app;
}

function digest(value: string): Buffer {
  return createHash('sha256').update(value).digest();
}
