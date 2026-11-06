'use client';

// An invoice being prepared in the Számlázó. The order screen no longer issues on the
// spot: "Számlázóba" files a draft, and here the office looks it over before anything is
// numbered or reported.
//
// The lines are the order's own items: editing one here edits the order, so the invoice and
// the order's parts list never disagree. The terms (rate, dates, payment method) are the
// invoice's alone and are kept on the draft as they are typed. The preview beside them is
// the backend's own arithmetic — what NAV would get — and lists whatever would stop the
// issue, so "Kiállítás" only lights up when nothing does.

import { useEffect, useState } from 'react';
import Link from 'next/link';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { invoicesApi, ordersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { useLookups } from '@/hooks/useLookups';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { canEditOrders, useAuth } from '@/lib/auth/context';
import { ItemsSection } from '@/components/forms/ItemsSection';
import { InvoicesSection } from '@/components/orders/InvoicesSection';
import { ConfirmDialog } from '@/components/ui/ConfirmDialog';
import { LoadingState } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import { Money } from '@/components/ui/Money';
import { DateDisplay } from '@/components/ui/DateDisplay';
import type { Currency, InvoicePreview, IssueRequest } from '@/lib/api/types';

type Terms = { vat_rate: string; issue_date: string; delivery_date: string; payment_date: string; payment_method: string };

const EMPTY: Terms = { vat_rate: '', issue_date: '', delivery_date: '', payment_date: '', payment_method: '' };

/** Blank fields are left to the server's defaults (today, the configured rate, transfer). */
function body(terms: Terms): IssueRequest {
  return {
    vat_rate: terms.vat_rate.trim() || null,
    issue_date: terms.issue_date || null,
    delivery_date: terms.delivery_date || null,
    payment_date: terms.payment_date || null,
    payment_method: terms.payment_method || null,
    send_email: true,
  };
}

export function InvoiceDraftPanel({ orderId, onClose }: { orderId: number; onClose: () => void }) {
  const t = useTranslations('invoices');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const locale = useLocale();
  const qc = useQueryClient();
  const { user } = useAuth();
  const { data: lookups } = useLookups();
  const methods = lookups?.invoice_payment_methods ?? [];
  const [error, setError] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [discarding, setDiscarding] = useState(false);
  const [issued, setIssued] = useState(false);

  const detail = useQuery({ queryKey: qk.order(orderId), queryFn: () => ordersApi.get(orderId) });
  const draft = useQuery({
    queryKey: ['invoice-draft', orderId],
    queryFn: () => invoicesApi.getDraft(orderId),
    retry: false,
  });

  // The terms as typed, seeded once from the stored draft.
  const [terms, setTerms] = useState<Terms | null>(null);
  useEffect(() => {
    if (terms !== null) return;
    if (draft.data) {
      setTerms({
        vat_rate: draft.data.vat_rate ?? '',
        issue_date: draft.data.issue_date ?? '',
        delivery_date: draft.data.delivery_date ?? '',
        payment_date: draft.data.payment_date ?? '',
        payment_method: draft.data.payment_method ?? '',
      });
    } else if (draft.isError) {
      setTerms(EMPTY);
    }
  }, [draft.data, draft.isError, terms]);

  // Kept on the draft as they are typed, so the next person sees the same invoice.
  const termsKey = JSON.stringify(terms ?? EMPTY);
  const savedKey = useDebouncedValue(termsKey, 600);
  const save = useMutation({
    mutationFn: (t: Terms) => invoicesApi.putDraft(orderId, body(t)),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ['invoice-drafts'] }),
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });
  const [lastSaved, setLastSaved] = useState<string | null>(null);
  useEffect(() => {
    if (terms === null || issued) return;
    if (lastSaved === null) {
      setLastSaved(savedKey);
      return;
    }
    if (savedKey !== lastSaved) {
      setLastSaved(savedKey);
      save.mutate(JSON.parse(savedKey) as Terms);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [savedKey, terms === null, issued]);

  // The preview follows the items (edited beside it) and the terms.
  const items = detail.data?.items ?? [];
  const itemsKey = items.map((i) => `${i.id}:${i.description}:${i.quantity}:${i.unit_price}`).join('|');
  const preview = useQuery({
    queryKey: ['invoice-preview', orderId, savedKey, itemsKey],
    queryFn: () => invoicesApi.preview(orderId, body(JSON.parse(savedKey) as Terms)),
    enabled: terms !== null && !issued,
    retry: false,
  });

  const issue = useMutation({
    mutationFn: () => invoicesApi.create(orderId, body(terms ?? EMPTY)),
    onSuccess: () => {
      setConfirming(false);
      setIssued(true);
      setError(null);
      void qc.invalidateQueries({ queryKey: ['invoice-drafts'] });
      void qc.invalidateQueries({ queryKey: qk.invoices(orderId) });
      void qc.invalidateQueries({ queryKey: ['invoices'] });
    },
    onError: (e) => {
      setConfirming(false);
      setError(errorMessage(e, ter, ter('unknownError')));
    },
  });

  const discard = useMutation({
    mutationFn: () => invoicesApi.deleteDraft(orderId),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['invoice-drafts'] });
      onClose();
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  if (detail.isLoading || terms === null) return <LoadingState />;
  if (detail.isError || !detail.data) return <ErrorState error={detail.error} onRetry={() => void detail.refetch()} />;
  const { order, partner } = detail.data;
  const currency = order.currency as Currency;
  const p = preview.data;
  const blocked = !p || p.problems.length > 0 || preview.isFetching || save.isPending;
  const set = (key: keyof Terms) => (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) =>
    setTerms((prev) => ({ ...(prev ?? EMPTY), [key]: e.target.value }));

  return (
    <section className="space-y-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <button type="button" className="text-metadata text-steel-500 underline" onClick={onClose}>
            ← {t('draftsBack')}
          </button>
          <h2 className="mt-1 text-section font-semibold">
            {t('draftTitle')}:{' '}
            <Link href={`/${locale}/orders/${order.id}`} className="underline">
              #{order.number} · {order.title}
            </Link>
          </h2>
          <p className="text-metadata text-steel-500">{partner.name}</p>
        </div>
        {!issued && (
          <div className="flex gap-2">
            <button type="button" className="btn-ghost btn-sm" onClick={() => setDiscarding(true)}>
              {t('draftDiscard')}
            </button>
            <button
              type="button"
              className="btn-primary btn-sm"
              disabled={blocked || issue.isPending}
              title={p && p.problems.length > 0 ? t('draftBlocked') : undefined}
              onClick={() => setConfirming(true)}
            >
              {issue.isPending ? tc('processing') : t('issue')}
            </button>
          </div>
        )}
      </div>

      {error && (
        <p className="text-body text-signal" role="alert">
          {error}
        </p>
      )}

      {issued ? (
        <div className="card p-4">
          <p className="text-body">{t('draftIssued')}</p>
          <div className="mt-4">
            <InvoicesSection orderId={order.id} currency={currency} />
          </div>
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-6 2xl:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
          <div className="space-y-5">
            <div>
              <h3 className="text-body font-semibold">{t('draftLines')}</h3>
              <p className="text-metadata text-steel-500">{t('draftLinesHint')}</p>
              <div className="mt-2">
                <ItemsSection orderId={order.id} items={items} currency={currency} readOnly={!canEditOrders(user)} />
              </div>
            </div>
            <div className="card p-4">
              <h3 className="text-body font-semibold">{t('draftTerms')}</h3>
              <div className="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-2">
                <div>
                  <label className="label" htmlFor="draft-vat">{t('vatRate')}</label>
                  <input id="draft-vat" className="input font-mono" inputMode="decimal" placeholder={p?.vat_rate ?? '0.27'} value={terms.vat_rate} onChange={set('vat_rate')} />
                  <p className="mt-1 text-metadata text-steel-500">{t('vatRateHint')}</p>
                </div>
                <div>
                  <label className="label" htmlFor="draft-method">{t('paymentMethod')}</label>
                  <select id="draft-method" className="input" value={terms.payment_method} onChange={set('payment_method')}>
                    <option value="">{t('draftDefault')}</option>
                    {methods.map((m) => (
                      <option key={m.key} value={m.key}>
                        {m.label_hu}
                      </option>
                    ))}
                  </select>
                </div>
                <div>
                  <label className="label" htmlFor="draft-issue">{t('issueDate')}</label>
                  <input id="draft-issue" type="date" className="input" value={terms.issue_date} onChange={set('issue_date')} />
                </div>
                <div>
                  <label className="label" htmlFor="draft-delivery">{t('deliveryDate')}</label>
                  <input id="draft-delivery" type="date" className="input" value={terms.delivery_date} onChange={set('delivery_date')} />
                </div>
                <div>
                  <label className="label" htmlFor="draft-payment">{t('paymentDate')}</label>
                  <input id="draft-payment" type="date" className="input" value={terms.payment_date} onChange={set('payment_date')} />
                </div>
              </div>
              <p className="mt-2 text-metadata text-steel-500">{t('draftTermsHint')}</p>
            </div>
          </div>

          <div>
            <h3 className="text-body font-semibold">{t('draftPreview')}</h3>
            {preview.isError ? (
              <p className="mt-2 text-body text-signal" role="alert">
                {errorMessage(preview.error, ter, ter('unknownError'))}
              </p>
            ) : !p ? (
              <LoadingState />
            ) : (
              <div className="mt-2 space-y-3">
                {p.problems.length > 0 && (
                  <ul className="space-y-1 rounded-lg border border-signal/40 bg-signal/5 p-3 text-body text-steel-900" role="alert">
                    {p.problems.map((m) => (
                      <li key={m}>• {m}</li>
                    ))}
                  </ul>
                )}
                {p.notes.map((m) => (
                  <p key={m} className="rounded-lg bg-steel-200/50 px-3 py-2 text-metadata text-steel-900">
                    {m}
                  </p>
                ))}
                <InvoicePaper preview={p} />
              </div>
            )}
          </div>
        </div>
      )}

      <ConfirmDialog
        open={confirming}
        title={t('issueTitle')}
        body={p ? t('draftConfirm', { number: order.number }) : ''}
        confirmLabel={t('issue')}
        busy={issue.isPending}
        onConfirm={() => issue.mutate()}
        onClose={() => setConfirming(false)}
      />
      <ConfirmDialog
        open={discarding}
        title={t('draftDiscardTitle')}
        body={t('draftDiscardBody')}
        confirmLabel={t('draftDiscard')}
        busy={discard.isPending}
        onConfirm={() => discard.mutate()}
        onClose={() => setDiscarding(false)}
      />
    </section>
  );
}

/** The preview laid out like the invoice itself: parties, dates, lines, totals. */
function InvoicePaper({ preview: p }: { preview: InvoicePreview }) {
  const t = useTranslations('invoices');
  const { data: lookups } = useLookups();
  const currency = p.currency as Currency;
  const method = lookups?.invoice_payment_methods?.find((m) => m.key === p.payment_method)?.label_hu ?? p.payment_method;
  return (
    <div className="card overflow-hidden">
      <div className="flex items-center justify-between bg-steel-900 px-4 py-3 text-white">
        <span className="font-semibold tracking-wide">{t('draftPaperTitle')}</span>
        <span className="text-metadata opacity-80">{t('draftPaperNotIssued')}</span>
      </div>
      <div className="grid grid-cols-2 gap-4 p-4 text-body">
        <Party label={t('supplier')} party={p.supplier} />
        <Party label={t('customer')} party={p.customer} />
      </div>
      <dl className="grid grid-cols-2 gap-x-4 gap-y-1 border-t border-steel-200 px-4 py-3 text-metadata sm:grid-cols-4">
        <PaperField label={t('issueDate')} value={<DateDisplay value={p.issue_date} />} />
        <PaperField label={t('deliveryDate')} value={<DateDisplay value={p.delivery_date} />} />
        <PaperField label={t('paymentDate')} value={<DateDisplay value={p.payment_date} />} />
        <PaperField label={t('paymentMethod')} value={method} />
      </dl>
      <div className="overflow-x-auto border-t border-steel-200">
        <table className="w-full text-body">
          <thead>
            <tr className="text-left text-metadata text-steel-500">
              <th className="px-4 py-2 font-medium">{t('lineDescription')}</th>
              <th className="px-2 py-2 text-right font-medium">{t('lineQuantity')}</th>
              <th className="px-2 py-2 text-right font-medium">{t('lineUnitPrice')}</th>
              <th className="px-2 py-2 text-right font-medium">{t('lineNet')}</th>
              <th className="px-4 py-2 text-right font-medium">{t('lineGross')}</th>
            </tr>
          </thead>
          <tbody>
            {p.lines.map((l) => (
              <tr key={l.item_id} className="border-t border-steel-200">
                <td className="px-4 py-2">{l.description}</td>
                <td className="px-2 py-2 text-right font-mono">{l.quantity}</td>
                <td className="px-2 py-2 text-right"><Money minor={l.unit_price_minor} currency={currency} /></td>
                <td className="px-2 py-2 text-right"><Money minor={l.net_minor} currency={currency} /></td>
                <td className="px-4 py-2 text-right"><Money minor={l.gross_minor} currency={currency} /></td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <dl className="space-y-1 border-t border-steel-200 px-4 py-3 text-body">
        <Total label={t('net')} minor={p.net_minor} currency={currency} />
        <Total label={`${t('vat')} (${(Number(p.vat_rate) * 100).toLocaleString('hu-HU')}%)`} minor={p.vat_minor} currency={currency} />
        <Total label={t('gross')} minor={p.gross_minor} currency={currency} strong />
      </dl>
    </div>
  );
}

function Party({ label, party }: { label: string; party: InvoicePreview['supplier'] }) {
  const t = useTranslations('invoices');
  return (
    <div>
      <p className="text-metadata font-medium text-steel-500">{label}</p>
      <p className="font-semibold">{party.name}</p>
      <p className="text-metadata">{party.address}</p>
      {party.tax_number && (
        <p className="text-metadata">
          {t('taxNumber')}: <span className="font-mono">{party.tax_number}</span>
        </p>
      )}
      {party.bank_account && (
        <p className="text-metadata">
          {t('bankAccount')}: <span className="font-mono">{party.bank_account}</span>
        </p>
      )}
    </div>
  );
}

function PaperField({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div>
      <dt className="text-steel-500">{label}</dt>
      <dd className="text-steel-900">{value}</dd>
    </div>
  );
}

function Total({ label, minor, currency, strong }: { label: string; minor: number; currency: Currency; strong?: boolean }) {
  return (
    <div className={`flex justify-between ${strong ? 'text-section font-semibold' : ''}`}>
      <dt>{label}</dt>
      <dd>
        <Money minor={minor} currency={currency} />
      </dd>
    </div>
  );
}
