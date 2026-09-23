import { buildInvoice, validateInvoice } from '@open-nav/core';
import type { InvoiceData } from '@open-nav/core';
import { renderInvoicePdf } from '@open-nav/invoicing';
import type { z } from 'zod';
import type { Config } from './config.js';
import type { ApiMessage } from './errors.js';
import { totalsOf } from './invoices.js';
import type { InvoiceTotals } from './invoices.js';
import type { proformaSchema } from './schemas.js';

/**
 * Proformas (díjbekérő).
 *
 * A proforma is a request for payment, not a tax invoice: it grants no VAT
 * deduction right and is reported nowhere. Nothing in this file talks to NAV —
 * no client, no credentials, no transaction. It builds the same document data
 * and hands it to the invoicing package's proforma renderer, which drops the
 * markings and provenance note a real invoice carries and prints the "not a tax
 * invoice" disclaimer instead.
 */

export interface ProformaResponse {
  id: string;
  documentType: 'proforma';
  invoiceNumber: string;
  /** Always false. A proforma is never reported to NAV. */
  reportedToNav: false;
  totals?: InvoiceTotals;
  /** The rendered PDF. Also retrievable at `GET /proformas/:id/pdf` until then. */
  pdfBase64: string;
  /** When this id stops resolving in the sidecar's in-memory cache, UTC. */
  expiresAt: string;
  /**
   * Findings from the local validator, for information only.
   *
   * A proforma is never submitted, so nothing here blocks it — but the data
   * usually becomes an invoice later, and hearing about a bad tax number now is
   * cheaper than hearing about it from NAV then.
   */
  messages: ApiMessage[];
}

export interface CachedProforma {
  id: string;
  invoiceNumber: string;
  pdf: Buffer;
  expiresAt: number;
}

/**
 * A bounded, in-memory hold on rendered proformas.
 *
 * Deliberately not a store. The sidecar has no database and keeps no durable
 * state; this exists so a caller can render in one call and fetch the bytes in
 * the next. Persisting the payload against an id is the caller's job — losing
 * this cache costs a re-POST, nothing more.
 */
export class ProformaCache {
  private readonly entries = new Map<string, CachedProforma>();

  constructor(
    private readonly ttlMs: number,
    private readonly max: number,
    private readonly now: () => number = Date.now,
  ) {}

  put(id: string, invoiceNumber: string, pdf: Buffer): CachedProforma {
    this.sweep();
    // Map preserves insertion order, so the first key is the oldest entry.
    while (this.entries.size >= this.max) {
      const oldest = this.entries.keys().next();
      if (oldest.done) break;
      this.entries.delete(oldest.value);
    }
    const entry: CachedProforma = {
      id,
      invoiceNumber,
      pdf,
      expiresAt: this.now() + this.ttlMs,
    };
    this.entries.delete(id);
    this.entries.set(id, entry);
    return entry;
  }

  get(id: string): CachedProforma | undefined {
    const entry = this.entries.get(id);
    if (!entry) return undefined;
    if (entry.expiresAt <= this.now()) {
      this.entries.delete(id);
      return undefined;
    }
    return entry;
  }

  get size(): number {
    return this.entries.size;
  }

  private sweep(): void {
    const now = this.now();
    for (const [id, entry] of this.entries) {
      if (entry.expiresAt <= now) this.entries.delete(id);
    }
  }
}

/** Build the document data a proforma prints from. Never sent anywhere. */
export function buildProformaDocument(payload: z.infer<typeof proformaSchema>): InvoiceData {
  return buildInvoice({
    invoiceNumber: payload.invoiceNumber,
    issueDate: payload.issueDate,
    deliveryDate: payload.deliveryDate,
    paymentDate: payload.paymentDate,
    currency: payload.currency,
    exchangeRate: payload.exchangeRate,
    appearance: payload.appearance,
    paymentMethod: payload.paymentMethod,
    priceMode: payload.priceMode,
    deliveryPeriod: payload.deliveryPeriod,
    orderNumbers: payload.orderNumbers,
    supplier: {
      name: payload.supplier.name,
      taxNumber: payload.supplier.taxNumber,
      bankAccount: payload.supplier.bankAccount,
      address: payload.supplier.address,
    },
    customer: {
      name: payload.customer.name,
      taxNumber: payload.customer.taxNumber,
      vatStatus: payload.customer.vatStatus,
      address: payload.customer.address,
    },
    lines: payload.lines.map((line) => ({
      description: line.description,
      quantity: line.quantity,
      unit: line.unit,
      unitPrice: line.unitPrice,
      vatPercentage: line.vatPercentage,
      nature: line.nature,
    })),
  });
}

/** Render a proforma to PDF. No NAV call happens anywhere on this path. */
export async function renderProforma(
  config: Config,
  cache: ProformaCache,
  payload: z.infer<typeof proformaSchema>,
): Promise<ProformaResponse> {
  const document = buildProformaDocument(payload);
  const id = payload.id ?? payload.invoiceNumber;

  const pdf = await renderInvoicePdf(document, {
    documentType: 'proforma',
    language: payload.language ?? config.pdfLanguage,
    ...(payload.note ? { note: payload.note } : {}),
  });

  const entry = cache.put(id, payload.invoiceNumber, pdf);
  const report = validateInvoice(document, { operation: 'CREATE' });

  return {
    id,
    documentType: 'proforma',
    invoiceNumber: payload.invoiceNumber,
    reportedToNav: false,
    ...(totalsOf(document) ? { totals: totalsOf(document) as InvoiceTotals } : {}),
    pdfBase64: pdf.toString('base64'),
    expiresAt: new Date(entry.expiresAt).toISOString(),
    messages: report.issues.map((issue) => ({
      source: 'local' as const,
      level: issue.severity === 'error' ? ('ERROR' as const) : ('WARN' as const),
      code: issue.code,
      message: issue.navMessage ? `${issue.message} (${issue.navMessage})` : issue.message,
      path: issue.path,
    })),
  };
}
