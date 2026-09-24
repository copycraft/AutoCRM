import { Router } from 'express';
import type { Request, Response } from 'express';
import { renderInvoicePdf } from '@open-nav/invoicing';
import type { InvoiceDirectionType } from '@open-nav/core';
import { badRequest } from '../errors.js';
import {
  annulInvoice,
  createInvoice,
  fetchInvoice,
  invoiceChain,
  stornoInvoice,
  totalsOf,
} from '../invoices.js';
import type { NavContext } from '../nav.js';
import {
  annulSchema,
  createInvoiceSchema,
  directionQuery,
  languageQuery,
  stornoSchema,
} from '../schemas.js';

/**
 * Everything under `/invoices` talks to NAV.
 *
 * Proformas are a separate router on purpose — see `routes/proformas.ts`.
 */
export function invoiceRoutes(context: NavContext): Router {
  const router = Router();

  /** Report a new invoice (operation CREATE) and wait for NAV's verdict. */
  router.post('/', async (request: Request, response: Response) => {
    const payload = createInvoiceSchema.parse(request.body);
    response.status(201).json(await createInvoice(context, payload));
  });

  /** Cancel an issued invoice in full (operation STORNO). */
  router.post('/:invoiceNumber/storno', async (request: Request, response: Response) => {
    const invoiceNumber = pathInvoiceNumber(request);
    const body = stornoSchema.parse(request.body);
    response.status(201).json(await stornoInvoice(context, invoiceNumber, body));
  });

  /** Technically annul an erroneous data report (manageAnnulment). */
  router.post('/:invoiceNumber/annul', async (request: Request, response: Response) => {
    const invoiceNumber = pathInvoiceNumber(request);
    const body = annulSchema.parse(request.body);
    response.status(201).json(await annulInvoice(context, invoiceNumber, body));
  });

  /** The invoice's modification/storno chain, as NAV records it. */
  router.get('/:invoiceNumber/chain', async (request: Request, response: Response) => {
    const invoiceNumber = pathInvoiceNumber(request);
    const direction = readDirection(request);
    const taxNumber = readString(request, 'taxNumber');
    response.json(await invoiceChain(context, invoiceNumber, direction, taxNumber));
  });

  /**
   * The invoice as a PDF, rendered from what NAV holds.
   *
   * Reading it back rather than re-rendering a local copy means the document
   * shows the data that was actually reported.
   */
  router.get('/:invoiceNumber/pdf', async (request: Request, response: Response) => {
    const invoiceNumber = pathInvoiceNumber(request);
    const direction = readDirection(request);
    const language = languageQuery.parse(readString(request, 'language')) ?? context.config.pdfLanguage;
    const supplierTaxNumber = readString(request, 'supplierTaxNumber');

    const invoice = await fetchInvoice(context, invoiceNumber, direction, supplierTaxNumber);
    const pdf = await renderInvoicePdf(invoice, { language });

    response.setHeader('Content-Type', 'application/pdf');
    response.setHeader(
      'Content-Disposition',
      `inline; filename="${safeFilename(invoiceNumber)}.pdf"`,
    );
    response.send(pdf);
  });

  /**
   * What NAV holds under a number: its issue date and totals, read back from NAV.
   *
   * For reconciling an unknown outcome. A caller whose submission timed out and whose
   * retry was answered INVOICE_NUMBER_ALREADY_EXISTS needs to know whether the stored
   * document is the one it sent, and this is the only authoritative copy.
   */
  router.get('/:invoiceNumber', async (request: Request, response: Response) => {
    const invoiceNumber = pathInvoiceNumber(request);
    const direction = readDirection(request);
    const supplierTaxNumber = readString(request, 'supplierTaxNumber');
    const invoice = await fetchInvoice(context, invoiceNumber, direction, supplierTaxNumber);
    const totals = totalsOf(invoice);
    response.json({
      invoiceNumber: invoice.invoiceNumber,
      issueDate: invoice.invoiceIssueDate,
      ...(totals ? { totals } : {}),
    });
  });

  return router;
}

/**
 * The invoice number from the path.
 *
 * NAV allows characters that need encoding in a URL (a slash, most awkwardly);
 * Express decodes the parameter, so callers percent-encode and this reads what
 * they meant.
 */
function pathInvoiceNumber(request: Request): string {
  const value = request.params.invoiceNumber;
  if (typeof value !== 'string' || value.length === 0) {
    throw badRequest('the path must name an invoice number');
  }
  return value;
}

function readDirection(request: Request): InvoiceDirectionType {
  return directionQuery.parse(readString(request, 'direction')) ?? 'OUTBOUND';
}

/** Query parameters arrive as string | string[] | object; take the simple case. */
function readString(request: Request, name: string): string | undefined {
  const value = request.query[name];
  if (typeof value === 'string' && value.length > 0) return value;
  if (value === undefined) return undefined;
  throw badRequest(`query parameter ${name} must be given at most once, as a string`);
}

function safeFilename(value: string): string {
  return value.replace(/[^A-Za-z0-9._-]/g, '_').slice(0, 80) || 'invoice';
}
