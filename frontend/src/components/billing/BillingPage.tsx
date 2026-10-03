'use client';

// Számlázó: the billing desk. History of every invoice and storno plus every
// díjbekérő across orders, and a create panel on the side: pick an order and
// the familiar per-order invoice/proforma sections open right there — issue,
// storno, annul, díjbekérő, and "invoice from this proforma" (which picks the
// proforma's order: the invoice is built from the order's line items, the
// same items the proforma rendered).

import { useState } from 'react';
import Link from 'next/link';
import { useLocale, useTranslations } from 'next-intl';
import { useQuery } from '@tanstack/react-query';
import { invoicesApi, ordersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { lookupLabel, useLookups } from '@/hooks/useLookups';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { errorMessage } from '@/lib/api/errors';
import { ErrorState } from '@/components/ui/ErrorState';
import { LoadingState } from '@/components/ui/LoadingState';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { Money } from '@/components/ui/Money';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { InvoicesSection } from '@/components/orders/InvoicesSection';
import { ProformasSection } from '@/components/orders/ProformasSection';
import type { BilledInvoice, BilledProforma, Currency, OrderSummary } from '@/lib/api/types';

const STATUSES = ['submitting', 'issued', 'rejected', 'stornoed', 'annulled'] as const;
const KINDS = ['invoice', 'storno'] as const;

function statusTone(status: string): 'done' | 'cold' | 'signal' | 'muted' | 'steel' {
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

/** A picked order is only id/number/partner/currency; the sections fetch the rest. */
function pickedOrder(
  id: number,
  number: string,
  partnerName: string,
  currency: string,
): OrderSummary {
  return { id, number, partner_name: partnerName, currency } as OrderSummary;
}

export function BillingPage() {
  const t = useTranslations('invoices');
  const locale = useLocale();
  const [tab, setTab] = useState<'invoices' | 'proformas'>('invoices');
  const [status, setStatus] = useState('');
  const [kind, setKind] = useState('');
  const [picked, setPicked] = useState<OrderSummary | null>(null);

  return (
    <div className="grid grid-cols-1 gap-6 xl:grid-cols-[1fr_380px]">
      <div>
        <div className="flex gap-2" role="tablist" aria-label={t('billingTitle')}>
          {(
            [
              ['invoices', t('billingHistory')],
              ['proformas', t('billingProformas')],
            ] as const
          ).map(([value, label]) => (
            <button
              key={value}
              role="tab"
              aria-selected={tab === value}
              className={tab === value ? 'btn-primary btn-sm' : 'btn-secondary btn-sm'}
              onClick={() => setTab(value)}
            >
              {label}
            </button>
          ))}
        </div>
        <div className="mt-4">
          {tab === 'invoices' ? (
            <InvoiceHistory
              status={status}
              kind={kind}
              onStatus={setStatus}
              onKind={setKind}
              onPickOrder={setPicked}
            />
          ) : (
            <ProformaHistory onPickOrder={setPicked} />
          )}
        </div>
        {picked && (
          <p className="mt-4 text-metadata text-steel-500">
            {t('billingOrder')}:{' '}
            <Link href={`/${locale}/orders/${picked.id}`} className="underline">
              #{picked.number} · {picked.title}
            </Link>
          </p>
        )}
      </div>
      <aside>
        <CreatePanel picked={picked} onPick={setPicked} onClear={() => setPicked(null)} />
      </aside>
    </div>
  );
}

function InvoiceHistory({
  status,
  kind,
  onStatus,
  onKind,
  onPickOrder,
}: {
  status: string;
  kind: string;
  onStatus: (v: string) => void;
  onKind: (v: string) => void;
  onPickOrder: (order: OrderSummary) => void;
}) {
  const t = useTranslations('invoices');
  const query = useQuery({
    queryKey: qk.invoicesAll({ status: status || undefined, kind: kind || undefined }),
    queryFn: () =>
      invoicesApi.all({
        status: status || undefined,
        kind: kind || undefined,
      }),
  });

  return (
    <section>
      <div className="flex flex-wrap items-center gap-2">
        <label className="label" htmlFor="billing-status">
          {t('billingStatus')}
        </label>
        <select
          id="billing-status"
          className="input w-auto"
          value={status}
          onChange={(e) => onStatus(e.target.value)}
        >
          <option value="">{t('billingAll')}</option>
          {STATUSES.map((s) => (
            <option key={s} value={s}>
              {t(`status.${s}`)}
            </option>
          ))}
        </select>
        <label className="label" htmlFor="billing-kind">
          {t('billingKind')}
        </label>
        <select
          id="billing-kind"
          className="input w-auto"
          value={kind}
          onChange={(e) => onKind(e.target.value)}
        >
          <option value="">{t('billingAll')}</option>
          <option value="invoice">{t('billingKindInvoice')}</option>
          <option value="storno">{t('kindStorno')}</option>
        </select>
      </div>
      <div className="mt-3">
        {query.isPending ? (
          <LoadingState />
        ) : query.isError ? (
          <ErrorState error={query.error} onRetry={() => void query.refetch()} />
        ) : query.data.items.length === 0 ? (
          <p className="text-body text-steel-500">{t('billingEmpty')}</p>
        ) : (
          <ul className="space-y-2">
            {query.data.items.map((invoice) => (
              <InvoiceRow key={invoice.id} invoice={invoice} onPickOrder={onPickOrder} />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}

function InvoiceRow({
  invoice,
  onPickOrder,
}: {
  invoice: BilledInvoice;
  onPickOrder: (order: OrderSummary) => void;
}) {
  const t = useTranslations('invoices');
  const locale = useLocale();
  const { data: lookups } = useLookups();
  return (
    <li className="card p-4">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
        <span className="font-mono text-body font-medium">{invoice.number}</span>
        <StatusBadge tone={statusTone(invoice.status)}>{t(`status.${invoice.status}`)}</StatusBadge>
        {invoice.kind === 'storno' && <StatusBadge tone="steel">{t('kindStorno')}</StatusBadge>}
        <Money minor={invoice.gross_amount} currency={invoice.currency as Currency} className="ml-auto" />
      </div>
      <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-metadata text-steel-500">
        <Link href={`/${locale}/orders/${invoice.order_id}`} className="underline">
          #{invoice.order_number}
        </Link>
        <span>{invoice.partner_name}</span>
        <span>{lookupLabel(lookups?.invoice_payment_methods, invoice.payment_method)}</span>
        <DateDisplay value={invoice.issue_date} />
        <button
          className="underline hover:text-steel-900"
          onClick={() =>
            onPickOrder(
              pickedOrder(
                invoice.order_id,
                invoice.order_number,
                invoice.partner_name,
                invoice.currency,
              ),
            )
          }
        >
          {t('issue')}
        </button>
      </div>
      {invoice.nav_error_code && (
        <p className="mt-1 font-mono text-metadata text-steel-900">
          {invoice.nav_error_code}: {invoice.nav_message}
        </p>
      )}
    </li>
  );
}

function ProformaHistory({ onPickOrder }: { onPickOrder: (order: OrderSummary) => void }) {
  const t = useTranslations('invoices');
  const query = useQuery({
    queryKey: qk.proformasAll,
    queryFn: () => invoicesApi.proformasAll(),
  });

  if (query.isPending) return <LoadingState />;
  if (query.isError) return <ErrorState error={query.error} onRetry={() => void query.refetch()} />;
  if (query.data.items.length === 0)
    return <p className="text-body text-steel-500">{t('billingProformaEmpty')}</p>;

  return (
    <section>
      <p className="text-metadata text-steel-500">{t('billingProformaHint')}</p>
      <ul className="mt-3 space-y-2">
        {query.data.items.map((proforma) => (
          <ProformaRow key={proforma.id} proforma={proforma} onPickOrder={onPickOrder} />
        ))}
      </ul>
    </section>
  );
}

function ProformaRow({
  proforma,
  onPickOrder,
}: {
  proforma: BilledProforma;
  onPickOrder: (order: OrderSummary) => void;
}) {
  const t = useTranslations('invoices');
  const locale = useLocale();
  return (
    <li className="card p-4">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
        <span className="font-mono text-body font-medium">{proforma.number}</span>
        <Money
          minor={proforma.gross_amount}
          currency={proforma.currency as Currency}
          className="ml-auto"
        />
      </div>
      <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-metadata text-steel-500">
        <Link href={`/${locale}/orders/${proforma.order_id}`} className="underline">
          #{proforma.order_number}
        </Link>
        <span>{proforma.partner_name}</span>
        <DateDisplay value={proforma.issue_date} />
        <button
          className="underline hover:text-steel-900"
          onClick={() =>
            onPickOrder(
              pickedOrder(
                proforma.order_id,
                proforma.order_number,
                proforma.partner_name,
                proforma.currency,
              ),
            )
          }
        >
          {t('billingIssueFromProforma')}
        </button>
      </div>
    </li>
  );
}

function CreatePanel({
  picked,
  onPick,
  onClear,
}: {
  picked: OrderSummary | null;
  onPick: (order: OrderSummary) => void;
  onClear: () => void;
}) {
  const t = useTranslations('invoices');
  const ter = useTranslations('errors');
  const [q, setQ] = useState('');
  const debouncedQ = useDebouncedValue(q);
  const search = useQuery({
    queryKey: qk.orders({ q: debouncedQ, billing: true }),
    queryFn: () => ordersApi.list({ q: debouncedQ || undefined, limit: 10 }),
    enabled: debouncedQ.trim().length > 1,
  });

  return (
    <section className="card p-4 lg:sticky lg:top-4">
      <h2 className="text-section font-semibold">{t('billingCreate')}</h2>
      <p className="mt-1 text-metadata text-steel-500">{t('billingPickHint')}</p>
      <div className="mt-3">
        <label className="label" htmlFor="billing-order-search">
          {t('billingPickOrder')}
        </label>
        <input
          id="billing-order-search"
          className="input"
          placeholder={t('billingSearchPlaceholder')}
          value={q}
          onChange={(e) => setQ(e.target.value)}
        />
        {search.isError && (
          <p className="mt-1 text-metadata text-steel-900" role="alert">
            {errorMessage(search.error, ter, ter('unknownError'))}
          </p>
        )}
        {search.data && search.data.items.length > 0 && (
          <ul className="mt-2 space-y-1">
            {search.data.items.map((order) => (
              <li key={order.id}>
                <button
                  className="w-full rounded-lg border border-steel-200 p-2 text-left hover:border-steel-900"
                  onClick={() => {
                    onPick(order);
                    setQ('');
                  }}
                >
                  <span className="font-mono text-body font-medium">#{order.number}</span>{' '}
                  <span className="text-body">{order.title}</span>
                  <span className="block text-metadata text-steel-500">{order.partner_name}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
      {picked && (
        <div className="mt-4 border-t border-steel-200 pt-4">
          <div className="flex items-center justify-between">
            <h3 className="text-body font-semibold">
              #{picked.number} · {picked.title ?? ''}
            </h3>
            <button className="btn-ghost btn-sm" onClick={onClear}>
              {t('billingRemove')}
            </button>
          </div>
          <div className="mt-3">
            <InvoicesSection orderId={picked.id} currency={picked.currency as Currency} />
          </div>
          <div className="mt-4">
            <ProformasSection orderId={picked.id} currency={picked.currency as Currency} />
          </div>
        </div>
      )}
    </section>
  );
}
