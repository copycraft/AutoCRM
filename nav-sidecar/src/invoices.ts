import {
  buildInvoice,
  buildStorno,
  decodeInvoiceData,
  toHeaderTimestamp,
  validateInvoice,
} from '@open-nav/core';
import type {
  InvoiceData,
  InvoiceDirectionType,
  InvoiceStatusType,
  ProcessingResultType,
} from '@open-nav/core';
import { waitForTransaction } from '@open-nav/client';
import { ApiError, notFound, resultMessages, validationFailed } from './errors.js';
import type { ApiMessage } from './errors.js';
import type { NavContext } from './nav.js';
import type { InvoicePayload } from './schemas.js';
import { z } from 'zod';
import type { annulSchema, stornoSchema } from './schemas.js';

/** What every reporting endpoint answers with. */
export interface SubmissionResponse {
  /** The document this reports on: the storno's own number for a storno. */
  invoiceNumber: string;
  operation: 'CREATE' | 'MODIFY' | 'STORNO' | 'ANNUL';
  /** NAV's transaction id. Worth storing: it is the handle for support. */
  transactionId: string;
  /** NAV's per-invoice status. `DONE` is stored, `ABORTED` is rejected. */
  status: InvoiceStatusType;
  accepted: boolean;
  /** NAV's messages, warnings included. Empty when it had nothing to say. */
  messages: ApiMessage[];
  /** Present when the sidecar built the document and therefore knows the money. */
  totals?: InvoiceTotals;
  /** Set on a storno: the invoice that was cancelled. */
  originalInvoiceNumber?: string;
}

export interface InvoiceTotals {
  currency: string;
  net: string;
  vat: string;
  gross: string;
}

export interface ChainResponse {
  invoiceNumber: string;
  direction: InvoiceDirectionType;
  elements: ChainElement[];
}

export interface ChainElement {
  invoiceNumber: string;
  /** `CREATE`, `MODIFY` or `STORNO`. */
  operation: string;
  supplierTaxNumber: string;
  customerTaxNumber?: string;
  /** When NAV recorded it, UTC. */
  insDate: string;
  batchIndex?: number;
  originalRequestVersion: string;
  /** Present on a modifying document: what it modifies. */
  modifies?: {
    originalInvoiceNumber: string;
    modificationIndex?: number;
    modifyWithoutMaster: boolean;
  };
}

/** Translate this service's payload into the NAV document, and check it. */
export function buildFromPayload(
  context: NavContext,
  payload: InvoicePayload,
  operation: 'CREATE' | 'MODIFY' | 'STORNO',
  what: string,
): InvoiceData {
  const invoice = buildInvoice({
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

  assertValid(context, invoice, operation, what);
  return invoice;
}

/**
 * Run the local validator and refuse to submit what NAV would reject.
 *
 * Warnings are not fatal — NAV validates tax numbers against its registry, so
 * treating a failed check digit as an error would refuse documents the service
 * accepts.
 */
function assertValid(
  context: NavContext,
  invoice: InvoiceData,
  operation: 'CREATE' | 'MODIFY' | 'STORNO',
  what: string,
): void {
  // The supplier must be the taxpayer the request authenticates as; NAV rejects
  // a report filed on someone else's behalf, so it is checked here first.
  const report = validateInvoice(invoice, {
    operation,
    supplierTaxNumber: context.config.credentials.taxNumber,
  });
  if (!report.valid) throw validationFailed(report.errors, what);
}

/** Money as the built document states it, for the caller's own records. */
export function totalsOf(invoice: InvoiceData): InvoiceTotals | undefined {
  const detail = invoice.invoiceMain.invoice?.invoiceHead.invoiceDetail;
  const summary = invoice.invoiceMain.invoice?.invoiceSummary;
  const normal = summary?.summaryNormal;
  if (!detail || !normal) return undefined;
  return {
    currency: detail.currencyCode,
    net: normal.invoiceNetAmount,
    vat: normal.invoiceVatAmount,
    gross: summary?.summaryGrossData?.invoiceGrossAmount ?? addAmounts(normal.invoiceNetAmount, normal.invoiceVatAmount),
  };
}

function addAmounts(left: string, right: string): string {
  return (Number(left) + Number(right)).toFixed(2);
}

/**
 * Submit one invoice and wait for NAV's verdict.
 *
 * `manageInvoice` returns a transaction id, not an answer; the answer arrives
 * later, per invoice. Polling here is what lets the Rust backend treat a
 * report as one synchronous call.
 */
async function submitOne(
  context: NavContext,
  operation: 'CREATE' | 'MODIFY' | 'STORNO',
  invoice: InvoiceData,
  originalInvoiceNumber?: string,
): Promise<SubmissionResponse> {
  const { transactionId } = await context.client.submitInvoices([{ operation, invoice }]);
  const outcome = await waitForTransaction(context.client, transactionId, {
    timeoutMs: context.config.pollTimeoutMs,
    initialDelayMs: context.config.pollInitialDelayMs,
  });

  const result = outcome.results[0];
  if (!result) {
    throw new ApiError({
      status: 502,
      kind: 'nav_error',
      message: `NAV returned no processing result for transaction ${transactionId}`,
      transactionId,
    });
  }

  const messages = resultMessages(result);
  if (result.invoiceStatus === 'ABORTED') {
    throw rejected(transactionId, result, messages);
  }

  return {
    invoiceNumber: invoice.invoiceNumber,
    operation,
    transactionId,
    status: result.invoiceStatus,
    accepted: true,
    messages,
    ...(totalsOf(invoice) ? { totals: totalsOf(invoice) as InvoiceTotals } : {}),
    ...(originalInvoiceNumber ? { originalInvoiceNumber } : {}),
  };
}

function rejected(
  transactionId: string,
  result: ProcessingResultType,
  messages: ApiMessage[],
): ApiError {
  const first = messages.find((message) => message.level === 'ERROR') ?? messages[0];
  return new ApiError({
    status: 422,
    kind: 'nav_rejected',
    message: first
      ? `NAV rejected the invoice: ${first.code ? `${first.code}: ` : ''}${first.message}`
      : 'NAV rejected the invoice without giving a reason',
    transactionId,
    messages,
  });
}

/** POST /invoices */
export async function createInvoice(
  context: NavContext,
  payload: InvoicePayload,
): Promise<SubmissionResponse> {
  const invoice = buildFromPayload(context, payload, 'CREATE', 'the invoice');
  return submitOne(context, 'CREATE', invoice);
}

/** POST /invoices/:invoiceNumber/storno */
export async function stornoInvoice(
  context: NavContext,
  originalInvoiceNumber: string,
  body: z.infer<typeof stornoSchema>,
): Promise<SubmissionResponse> {
  // The original is either supplied by the caller or read back from NAV; the
  // sidecar keeps no invoice store of its own.
  const original = body.original
    ? buildFromPayload(context, body.original, 'CREATE', 'the original invoice')
    : await fetchInvoice(context, originalInvoiceNumber, 'OUTBOUND');

  if (original.invoiceNumber !== originalInvoiceNumber) {
    throw new ApiError({
      status: 400,
      kind: 'bad_request',
      message:
        `original.invoiceNumber is ${JSON.stringify(original.invoiceNumber)} ` +
        `but the path names ${JSON.stringify(originalInvoiceNumber)}`,
    });
  }

  const storno = buildStorno(original, {
    invoiceNumber: body.stornoInvoiceNumber,
    issueDate: body.issueDate,
    modificationIndex: body.modificationIndex,
    chainLineBase: body.chainLineBase,
  });
  assertValid(context, storno, 'STORNO', 'the storno');

  return submitOne(context, 'STORNO', storno, originalInvoiceNumber);
}

/** POST /invoices/:invoiceNumber/annul */
export async function annulInvoice(
  context: NavContext,
  invoiceNumber: string,
  body: z.infer<typeof annulSchema>,
): Promise<SubmissionResponse> {
  const { transactionId } = await context.client.submitAnnulments([
    {
      annulmentReference: invoiceNumber,
      annulmentTimestamp: toHeaderTimestamp(),
      annulmentCode: body.code,
      annulmentReason: body.reason,
    },
  ]);

  const outcome = await waitForTransaction(context.client, transactionId, {
    timeoutMs: context.config.pollTimeoutMs,
    initialDelayMs: context.config.pollInitialDelayMs,
  });

  const result = outcome.results[0];
  if (!result) {
    throw new ApiError({
      status: 502,
      kind: 'nav_error',
      message: `NAV returned no processing result for annulment transaction ${transactionId}`,
      transactionId,
    });
  }

  const messages = resultMessages(result);
  if (result.invoiceStatus === 'ABORTED') throw rejected(transactionId, result, messages);

  return {
    invoiceNumber,
    operation: 'ANNUL',
    transactionId,
    status: result.invoiceStatus,
    accepted: true,
    messages,
  };
}

/**
 * Read one invoice back from NAV.
 *
 * NAV stores what was reported, so this is the authoritative copy — which is
 * why the PDF and the storno are rendered from it rather than from anything
 * the sidecar remembers.
 */
export async function fetchInvoice(
  context: NavContext,
  invoiceNumber: string,
  direction: InvoiceDirectionType,
  supplierTaxNumber?: string,
): Promise<InvoiceData> {
  const response = await context.client.queryInvoiceData({
    invoiceNumberQuery: {
      invoiceNumber,
      invoiceDirection: direction,
      // Only meaningful on an inbound query, and an error on an outbound one.
      ...(direction === 'INBOUND' && supplierTaxNumber ? { supplierTaxNumber } : {}),
    },
  });

  const found = response.invoiceDataResult;
  if (!found) {
    throw notFound(`NAV has no ${direction.toLowerCase()} invoice numbered ${invoiceNumber}`);
  }

  return decodeInvoiceData(found.invoiceData, {
    compressed: found.compressedContentIndicator,
  });
}

/** GET /invoices/:invoiceNumber/chain */
export async function invoiceChain(
  context: NavContext,
  invoiceNumber: string,
  direction: InvoiceDirectionType,
  taxNumber?: string,
): Promise<ChainResponse> {
  const elements: ChainElement[] = [];
  let page = 1;

  for (;;) {
    const response = await context.client.queryInvoiceChainDigest({
      page,
      invoiceChainQuery: {
        invoiceNumber,
        invoiceDirection: direction,
        // As with the digest queries, the counterparty's tax number is the
        // inbound search criterion and is not expected on an outbound one.
        ...(direction === 'INBOUND' && taxNumber ? { taxNumber } : {}),
      },
    });

    const result = response.invoiceChainDigestResult;
    for (const element of result.invoiceChainElement ?? []) {
      const digest = element.invoiceChainDigest;
      const reference = element.invoiceReferenceData;
      elements.push({
        invoiceNumber: digest.invoiceNumber,
        operation: digest.invoiceOperation,
        supplierTaxNumber: digest.supplierTaxNumber,
        ...(digest.customerTaxNumber ? { customerTaxNumber: digest.customerTaxNumber } : {}),
        insDate: digest.insDate,
        ...(digest.batchIndex === undefined ? {} : { batchIndex: digest.batchIndex }),
        originalRequestVersion: digest.originalRequestVersion,
        ...(reference
          ? {
              modifies: {
                originalInvoiceNumber: reference.originalInvoiceNumber,
                ...(reference.modificationIndex === undefined
                  ? {}
                  : { modificationIndex: reference.modificationIndex }),
                modifyWithoutMaster: reference.modifyWithoutMaster,
              },
            }
          : {}),
      });
    }

    if (!result.availablePage || result.currentPage >= result.availablePage) break;
    page = result.currentPage + 1;
  }

  return { invoiceNumber, direction, elements };
}
