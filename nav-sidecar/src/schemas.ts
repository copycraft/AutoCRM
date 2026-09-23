import { z } from 'zod';

/**
 * The HTTP boundary.
 *
 * These shapes are deliberately flat and stable: a Rust client is generated
 * against them, so they stay one or two levels deep and never mirror NAV's own
 * schema. Everything NAV-shaped — `invoiceMain.invoice.invoiceHead`,
 * `summaryByVatRate`, base64 payloads, signatures — is built inside the
 * sidecar from these.
 *
 * Every object is `.strict()`. An unknown key is a 400 rather than a silent
 * omission, which is what makes the contract safe to generate against, and it
 * is also what stops a caller smuggling in credentials: those come from the
 * environment only.
 */

/** Money and quantities: a number, or a decimal string when precision matters. */
const amount = z.union([
  z.number().finite(),
  z.string().regex(/^-?\d+(\.\d+)?$/, 'expected a decimal number, e.g. "1234.56"'),
]);

const isoDate = z
  .string()
  .regex(/^\d{4}-\d{2}-\d{2}$/, 'expected a date as yyyy-mm-dd');

export const unitOfMeasure = z.enum([
  'PIECE',
  'KILOGRAM',
  'TON',
  'KWH',
  'DAY',
  'HOUR',
  'MINUTE',
  'MONTH',
  'LITER',
  'KILOMETER',
  'CUBIC_METER',
  'METER',
  'LINEAR_METER',
  'CARTON',
  'PACK',
  'OWN',
]);

export const addressSchema = z
  .object({
    /** ISO 3166 alpha-2. Defaults to HU. */
    countryCode: z.string().length(2).optional(),
    postalCode: z.string().min(1).max(10),
    city: z.string().min(1),
    /** Street name without the category: "Kossuth", not "Kossuth utca". */
    streetName: z.string().min(1),
    /** "utca", "út", "tér", … */
    publicPlaceCategory: z.string().min(1),
    /** House number, as printed. */
    number: z.string().min(1),
  })
  .strict();

export const supplierSchema = z
  .object({
    name: z.string().min(1),
    /** 8- or 11-digit Hungarian tax number. Required for the supplier. */
    taxNumber: z.string().min(8),
    bankAccount: z.string().min(1).optional(),
    address: addressSchema,
  })
  .strict();

export const customerSchema = z
  .object({
    name: z.string().min(1),
    /** Omit for a private individual. */
    taxNumber: z.string().min(8).optional(),
    /** Defaults to DOMESTIC with a tax number, PRIVATE_PERSON without one. */
    vatStatus: z.enum(['DOMESTIC', 'OTHER', 'PRIVATE_PERSON']).optional(),
    address: addressSchema,
  })
  .strict();

export const lineSchema = z
  .object({
    description: z.string().min(1),
    quantity: amount,
    unit: unitOfMeasure.optional(),
    /** Net by default; gross when the invoice sets `priceMode: "gross"`. */
    unitPrice: amount,
    /**
     * VAT rate as a fraction: 0.27, not 27. Rejected above 1 rather than
     * guessed at, because guessing wrong on a tax rate is not recoverable.
     */
    vatPercentage: amount.refine(
      (value) => Number(value) >= 0 && Number(value) <= 1,
      'vatPercentage is a fraction: use 0.27 for 27%',
    ),
    nature: z.enum(['PRODUCT', 'SERVICE', 'OTHER']).optional(),
  })
  .strict();

/**
 * One invoice, as this service takes it.
 *
 * The same shape serves invoices and proformas — a díjbekérő carries exactly
 * the same data, it is simply never reported.
 */
export const invoiceSchema = z
  .object({
    invoiceNumber: z.string().min(1).max(50),
    issueDate: isoDate,
    /** Fulfilment date. Defaults to the issue date. */
    deliveryDate: isoDate.optional(),
    paymentDate: isoDate.optional(),
    /** ISO 4217. Defaults to HUF. */
    currency: z.string().length(3).optional(),
    /** Rate to HUF. Required for a non-HUF invoice. */
    exchangeRate: amount.optional(),
    appearance: z.enum(['PAPER', 'ELECTRONIC', 'EDI', 'UNKNOWN']).optional(),
    paymentMethod: z.enum(['TRANSFER', 'CASH', 'CARD', 'VOUCHER', 'OTHER']).optional(),
    /** Whether `unitPrice` on each line is net (default) or gross. */
    priceMode: z.enum(['net', 'gross']).optional(),
    /** Continuous settlement period ("folyamatos teljesítés"). */
    deliveryPeriod: z.object({ start: isoDate, end: isoDate }).strict().optional(),
    orderNumbers: z.array(z.string().min(1)).optional(),
    supplier: supplierSchema,
    customer: customerSchema,
    lines: z.array(lineSchema).min(1).max(100),
  })
  .strict();

export type InvoicePayload = z.infer<typeof invoiceSchema>;

/** POST /invoices */
export const createInvoiceSchema = invoiceSchema;

/** POST /invoices/:invoiceNumber/storno */
export const stornoSchema = z
  .object({
    /** Number of the storno document itself — not the original's. */
    stornoInvoiceNumber: z.string().min(1).max(50),
    /** Issue date of the storno. Defaults to the original's. */
    issueDate: isoDate.optional(),
    /**
     * The invoice being cancelled, in this service's own shape.
     *
     * Omit it and the sidecar fetches the original from NAV, which is the
     * simpler path: it keeps no invoice store of its own. Pass it when the
     * original was never reported through this sidecar, or when the caller
     * already holds the authoritative copy.
     */
    original: invoiceSchema.optional(),
    /** Position of this cancellation in the chain. Defaults to 1. */
    modificationIndex: z.number().int().positive().optional(),
    /** Lines already in the chain. Defaults to the original's line count. */
    chainLineBase: z.number().int().nonnegative().optional(),
  })
  .strict();

/** POST /invoices/:invoiceNumber/annul */
export const annulSchema = z
  .object({
    code: z.enum([
      'ERRATIC_DATA',
      'ERRATIC_INVOICE_NUMBER',
      'ERRATIC_INVOICE_ISSUE_DATE',
      'ERRATIC_ELECTRONIC_HASH_VALUE',
    ]),
    /** Why, in plain words. NAV stores and displays it. */
    reason: z.string().min(1).max(1024),
  })
  .strict();

/** POST /proformas — the same invoice payload, plus rendering options. */
export const proformaSchema = invoiceSchema
  .extend({
    /**
     * Identifier the caller wants this proforma retrievable by. Defaults to
     * `invoiceNumber`. Nothing is persisted: see the README.
     */
    id: z.string().min(1).max(100).optional(),
    /** Extra note printed under the totals, e.g. payment instructions. */
    note: z.string().max(2000).optional(),
    language: z.enum(['hu', 'en', 'de']).optional(),
  })
  .strict();

export const directionQuery = z.enum(['OUTBOUND', 'INBOUND']).optional();
export const languageQuery = z.enum(['hu', 'en', 'de']).optional();
