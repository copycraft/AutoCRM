'use client';

// Invoices on the order screen: issue, storno, annul, and what NAV said about each.
//
// Reporting is asynchronous — the backend answers 202 and a background job waits on the
// tax authority — so an invoice that is `submitting` is polled until NAV decides, and the
// row says so rather than looking inert. A rejection shows NAV's own fault code: it is the
// only thing that tells the office what to fix, so it is never flattened into "hiba".
//
// Proformas are NOT here. A díjbekérő is reported nowhere and has no status, no chain and
// no NAV transaction, so it lives in its own section (`ProformasSection`).

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { invoicesApi, mediaApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { canAdmin, canEditOrders, useAuth } from '@/lib/auth/context';
import { Money } from '@/components/ui/Money';
import { StatusBadge, type StatusTone } from '@/components/ui/StatusBadge';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { ConfirmDialog } from '@/components/ui/ConfirmDialog';
import type { Currency, Invoice, InvoiceStatus, NavMessage } from '@/lib/api/types';

/** How often to ask NAV's verdict while an invoice is in flight. */
const POLL_MS = 3000;

const ANNULMENT_CODES = [
  'ERRATIC_DATA',
  'ERRATIC_INVOICE_NUMBER',
  'ERRATIC_INVOICE_ISSUE_DATE',
  'ERRATIC_ELECTRONIC_HASH_VALUE',
] as const;

function statusTone(status: InvoiceStatus): StatusTone {
  switch (status) {
    case 'issued':
      return 'done';
    case 'submitting':
      return 'cold';
    case 'rejected':
      return 'signal';
    case 'stornoed':
    case 'annulled':
      return 'muted';
    default:
      return 'steel';
  }
}

/** The NAV messages worth showing: the errors, and warnings on a stored invoice. */
function navMessages(invoice: Invoice): NavMessage[] {
  return (invoice.nav_messages ?? []).filter((m) => m.level === 'ERROR' || m.level === 'WARN');
}

export function InvoicesSection({
  orderId,
  currency,
}: {
  orderId: number;
  currency: Currency;
}) {
  const t = useTranslations('invoices');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const { user } = useAuth();
  const qc = useQueryClient();
  const canIssue = canEditOrders(user);
  const canAnnul = canAdmin(user);

  const [issueOpen, setIssueOpen] = useState(false);
  const [stornoFor, setStornoFor] = useState<Invoice | null>(null);
  const [annulFor, setAnnulFor] = useState<Invoice | null>(null);
  const [error, setError] = useState<string | null>(null);

  const invoices = useQuery({
    queryKey: qk.invoices(orderId),
    queryFn: () => invoicesApi.forOrder(orderId),
    // While NAV is deciding, the answer arrives on its own rather than on a reload.
    refetchInterval: (query) =>
      query.state.data?.items.some((i) => i.status === 'submitting') ? POLL_MS : false,
  });

  const items = invoices.data?.items ?? [];
  const live = items.find((i) => i.kind === 'invoice' && i.status === 'issued');
  const inFlight = items.some((i) => i.status === 'submitting');

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: qk.invoices(orderId) });
    void qc.invalidateQueries({ queryKey: qk.documents(orderId) });
  };

  const onError = (e: unknown) => setError(errorMessage(e, ter, ter('unknownError')));

  const issue = useMutation({
    mutationFn: (body: { vat_rate?: string; payment_date?: string }) =>
      invoicesApi.create(orderId, body),
    onSuccess: () => {
      setIssueOpen(false);
      setError(null);
      refresh();
    },
    onError,
  });

  const storno = useMutation({
    mutationFn: (invoice: Invoice) => invoicesApi.storno(invoice.id, {}),
    onSuccess: () => {
      setStornoFor(null);
      setError(null);
      refresh();
    },
    onError,
  });

  const annul = useMutation({
    mutationFn: ({ invoice, code, reason }: { invoice: Invoice; code: string; reason: string }) =>
      invoicesApi.annul(invoice.id, { code, reason }),
    onSuccess: () => {
      setAnnulFor(null);
      setError(null);
      refresh();
    },
    onError,
  });

  return (
    <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
      <div className="flex items-center justify-between">
        <h2 className="text-section font-semibold">{t('title')}</h2>
        {canIssue && (
          <button
            className="btn-secondary btn-sm"
            onClick={() => setIssueOpen(true)}
            disabled={inFlight || !!live}
            title={live ? t('liveInvoiceExists') : inFlight ? t('submittingHint') : undefined}
          >
            {t('issue')}
          </button>
        )}
      </div>

      {error && (
        <p className="mt-3 text-body text-signal" role="alert">
          {error}
        </p>
      )}

      {invoices.isLoading ? (
        <p className="mt-3 text-body text-steel-500">{tc('loading')}</p>
      ) : items.length === 0 ? (
        <p className="mt-3 text-body text-steel-500">{t('none')}</p>
      ) : (
        <ul className="mt-3 divide-y divide-steel-200">
          {items.map((invoice) => (
            <InvoiceRow
              key={invoice.id}
              invoice={invoice}
              currency={currency}
              canIssue={canIssue}
              canAnnul={canAnnul}
              onStorno={() => setStornoFor(invoice)}
              onAnnul={() => setAnnulFor(invoice)}
            />
          ))}
        </ul>
      )}

      {issueOpen && (
        <IssueDialog
          busy={issue.isPending}
          onClose={() => setIssueOpen(false)}
          onSubmit={(body) => issue.mutate(body)}
        />
      )}

      <ConfirmDialog
        open={!!stornoFor}
        title={t('stornoTitle')}
        body={stornoFor ? t('stornoBody', { number: stornoFor.number }) : undefined}
        confirmLabel={t('storno')}
        busy={storno.isPending}
        onConfirm={() => stornoFor && storno.mutate(stornoFor)}
        onClose={() => setStornoFor(null)}
      />

      {annulFor && (
        <AnnulDialog
          number={annulFor.number}
          busy={annul.isPending}
          onClose={() => setAnnulFor(null)}
          onSubmit={(code, reason) => annul.mutate({ invoice: annulFor, code, reason })}
        />
      )}
    </section>
  );
}

function InvoiceRow({
  invoice,
  currency,
  canIssue,
  canAnnul,
  onStorno,
  onAnnul,
}: {
  invoice: Invoice;
  currency: Currency;
  canIssue: boolean;
  canAnnul: boolean;
  onStorno: () => void;
  onAnnul: () => void;
}) {
  const t = useTranslations('invoices');
  const [chainOpen, setChainOpen] = useState(false);
  const messages = navMessages(invoice);

  return (
    <li className="py-3">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
        <span className="font-mono text-body font-medium">{invoice.number}</span>
        <StatusBadge tone={statusTone(invoice.status)}>
          {t(`status.${invoice.status}`)}
        </StatusBadge>
        {invoice.kind === 'storno' && (
          <StatusBadge tone="steel">{t('kindStorno')}</StatusBadge>
        )}
        <DateDisplay value={invoice.issue_date} className="text-metadata text-steel-500" />
        <Money minor={invoice.gross_amount} currency={currency} className="ml-auto" />
      </div>

      <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-metadata text-steel-500">
        {invoice.nav_transaction_id && (
          <span className="font-mono" title={t('transactionId')}>
            {invoice.nav_transaction_id}
          </span>
        )}
        {invoice.status === 'submitting' && <span>{t('submittingHint')}</span>}
        {invoice.document_id && <DocumentLink documentId={invoice.document_id} label={t('pdf')} />}
        {invoice.status !== 'submitting' && (
          <button className="underline hover:text-steel-900" onClick={() => setChainOpen((open) => !open)}>
            {chainOpen ? t('hideChain') : t('showChain')}
          </button>
        )}
        {canIssue && invoice.kind === 'invoice' && invoice.status === 'issued' && (
          <button className="underline hover:text-steel-900" onClick={onStorno}>
            {t('storno')}
          </button>
        )}
        {canAnnul && (invoice.status === 'issued' || invoice.status === 'stornoed') && (
          <button className="underline hover:text-steel-900" onClick={onAnnul}>
            {t('annul')}
          </button>
        )}
      </div>

      {invoice.status === 'rejected' && (
        <div className="mt-2 rounded border border-signal/30 bg-signal/5 p-2">
          <p className="text-body font-medium text-signal">
            {t('rejected')}
            {invoice.nav_error_code ? ` — ${invoice.nav_error_code}` : ''}
          </p>
          {invoice.nav_message && <p className="text-metadata">{invoice.nav_message}</p>}
        </div>
      )}

      {messages.length > 0 && (
        <ul className="mt-1 space-y-0.5">
          {messages.map((message, index) => (
            <li key={index} className="text-metadata text-steel-500">
              <span className="font-mono">{message.code ?? message.level}</span> — {message.message}
              {message.path ? ` (${message.path})` : ''}
            </li>
          ))}
        </ul>
      )}

      {chainOpen && <InvoiceChain invoiceId={invoice.id} />}
    </li>
  );
}

/** The chain as NAV records it: the original, and everything that modified it. */
function InvoiceChain({ invoiceId }: { invoiceId: number }) {
  const t = useTranslations('invoices');
  const ter = useTranslations('errors');
  const chain = useQuery({
    queryKey: qk.invoiceChain(invoiceId),
    queryFn: () => invoicesApi.chain(invoiceId),
    retry: false,
  });

  if (chain.isLoading) return <p className="mt-2 text-metadata text-steel-500">{t('chainLoading')}</p>;
  if (chain.isError) {
    return (
      <p className="mt-2 text-metadata text-steel-500">
        {errorMessage(chain.error, ter, ter('unknownError'))}
      </p>
    );
  }
  const items = chain.data?.items ?? [];
  if (items.length === 0) {
    return <p className="mt-2 text-metadata text-steel-500">{t('chainEmpty')}</p>;
  }
  return (
    <ol className="mt-2 space-y-1 border-l border-steel-200 pl-3">
      {items.map((step, index) => (
        <li key={`${step.invoice_number}-${index}`} className="text-metadata">
          <span className="font-mono">{step.invoice_number}</span>{' '}
          <span className="text-steel-500">{t(`operation.${step.operation}`)}</span>
          {step.original_invoice_number && (
            <span className="text-steel-500"> → {step.original_invoice_number}</span>
          )}
        </li>
      ))}
    </ol>
  );
}

/** Opens the stored PDF through a presigned link, the same way any document opens. */
export function DocumentLink({ documentId, label }: { documentId: number; label: string }) {
  const [busy, setBusy] = useState(false);
  const open = async () => {
    setBusy(true);
    try {
      const { url } = await mediaApi.downloadDocument(documentId);
      window.open(url, '_blank', 'noopener,noreferrer');
    } finally {
      setBusy(false);
    }
  };
  return (
    <button className="underline hover:text-steel-900" onClick={() => void open()} disabled={busy}>
      {label}
    </button>
  );
}

/**
 * Issuing an invoice is a confirmation, not a form: the order already carries the lines,
 * the customer and the currency. The two fields that are ever worth overriding are here,
 * both pre-filled by the server when left alone.
 */
function IssueDialog({
  busy,
  onClose,
  onSubmit,
}: {
  busy: boolean;
  onClose: () => void;
  onSubmit: (body: { vat_rate?: string; payment_date?: string }) => void;
}) {
  const t = useTranslations('invoices');
  const tc = useTranslations('common');
  const [vatRate, setVatRate] = useState('');
  const [paymentDate, setPaymentDate] = useState('');

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-steel-900/40 p-4">
      <div className="card w-full max-w-md" role="dialog" aria-modal="true" aria-label={t('issue')}>
        <div className="card-header">
          <h3 className="text-section font-semibold">{t('issueTitle')}</h3>
        </div>
        <div className="card-content space-y-3">
          <p className="text-body text-steel-500">{t('issueBody')}</p>
          <div>
            <label className="label" htmlFor="invoice-vat">
              {t('vatRate')}
            </label>
            <input
              id="invoice-vat"
              className="input font-mono"
              inputMode="decimal"
              placeholder="0.27"
              value={vatRate}
              onChange={(e) => setVatRate(e.target.value)}
            />
            <p className="mt-1 text-metadata text-steel-500">{t('vatRateHint')}</p>
          </div>
          <div>
            <label className="label" htmlFor="invoice-payment-date">
              {t('paymentDate')}
            </label>
            <input
              id="invoice-payment-date"
              type="date"
              className="input"
              value={paymentDate}
              onChange={(e) => setPaymentDate(e.target.value)}
            />
          </div>
        </div>
        <div className="card-footer justify-end">
          <button className="btn-ghost" onClick={onClose} disabled={busy}>
            {tc('cancel')}
          </button>
          <button
            className="btn-primary"
            disabled={busy}
            onClick={() =>
              onSubmit({
                ...(vatRate.trim() ? { vat_rate: vatRate.trim() } : {}),
                ...(paymentDate ? { payment_date: paymentDate } : {}),
              })
            }
          >
            {busy ? tc('processing') : t('issue')}
          </button>
        </div>
      </div>
    </div>
  );
}

/**
 * A technical annulment says the *report* should never have existed, which is a different
 * claim from "the invoice was wrong" — that is a storno. NAV wants a coded reason and a
 * written one, and a person approves it afterwards in the Online Számla portal, so the
 * dialog asks for both and says as much.
 */
function AnnulDialog({
  number,
  busy,
  onClose,
  onSubmit,
}: {
  number: string;
  busy: boolean;
  onClose: () => void;
  onSubmit: (code: string, reason: string) => void;
}) {
  const t = useTranslations('invoices');
  const tc = useTranslations('common');
  const [code, setCode] = useState<string>(ANNULMENT_CODES[0]);
  const [reason, setReason] = useState('');

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-steel-900/40 p-4">
      <div className="card w-full max-w-md" role="dialog" aria-modal="true" aria-label={t('annul')}>
        <div className="card-header">
          <h3 className="text-section font-semibold">{t('annulTitle')}</h3>
        </div>
        <div className="card-content space-y-3">
          <p className="text-body">{t('annulBody', { number })}</p>
          <div>
            <label className="label" htmlFor="annul-code">
              {t('annulCode')}
            </label>
            <select
              id="annul-code"
              className="input"
              value={code}
              onChange={(e) => setCode(e.target.value)}
            >
              {ANNULMENT_CODES.map((value) => (
                <option key={value} value={value}>
                  {t(`annulCodes.${value}`)}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="label" htmlFor="annul-reason">
              {t('annulReason')} *
            </label>
            <textarea
              id="annul-reason"
              className="input"
              rows={3}
              value={reason}
              onChange={(e) => setReason(e.target.value)}
            />
          </div>
        </div>
        <div className="card-footer justify-end">
          <button className="btn-ghost" onClick={onClose} disabled={busy}>
            {tc('cancel')}
          </button>
          <button
            className="btn-primary"
            disabled={busy || reason.trim().length === 0}
            onClick={() => onSubmit(code, reason.trim())}
          >
            {busy ? tc('processing') : t('annul')}
          </button>
        </div>
      </div>
    </div>
  );
}
