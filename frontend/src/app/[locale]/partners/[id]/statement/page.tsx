'use client';

// A partner's statement of account (folyószámla-kivonat / egyenlegközlő): the invoices of
// the period, older ones still open, the payments received, and the balance per currency.
// Printable, so it can go to the customer as a PDF.

import { useState } from 'react';
import Link from 'next/link';
import { useLocale, useTranslations } from 'next-intl';
import { useQuery } from '@tanstack/react-query';
import { Printer } from 'lucide-react';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { DetailSkeleton } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import { Money } from '@/components/ui/Money';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { paymentsApi } from '@/lib/api/endpoints';
import type { Currency } from '@/lib/api/types';

function isoDay(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

export default function StatementPage({ params }: { params: { id: string } }) {
  const id = Number(params.id);
  const t = useTranslations('statement');
  const locale = useLocale();
  const today = new Date();
  const [from, setFrom] = useState(isoDay(new Date(today.getFullYear() - 1, today.getMonth(), today.getDate())));
  const [to, setTo] = useState(isoDay(today));
  const q = useQuery({
    queryKey: ['partners', id, 'statement', from, to],
    queryFn: () => paymentsApi.statement(id, { from, to }),
  });

  return (
    <AppShell>
      <div className="flex flex-wrap items-end justify-between gap-3 print:hidden">
        <PageHeader title={t('title')} />
        <div className="flex flex-wrap items-end gap-2">
          <div>
            <label className="label" htmlFor="st-from">{t('from')}</label>
            <input id="st-from" type="date" className="input" value={from} onChange={(e) => setFrom(e.target.value)} />
          </div>
          <div>
            <label className="label" htmlFor="st-to">{t('to')}</label>
            <input id="st-to" type="date" className="input" value={to} onChange={(e) => setTo(e.target.value)} />
          </div>
          <button className="btn-secondary" onClick={() => window.print()}>
            <Printer className="h-4 w-4" aria-hidden />
            {t('print')}
          </button>
        </div>
      </div>
      {q.isLoading ? (
        <DetailSkeleton />
      ) : q.isError || !q.data ? (
        <ErrorState error={q.error} onRetry={() => void q.refetch()} />
      ) : (
        <article className="card space-y-6 p-6 print:border-0 print:p-0 print:shadow-none">
          <header className="flex flex-wrap justify-between gap-4">
            <div>
              <h1 className="text-page-title">{t('documentTitle')}</h1>
              <p className="text-metadata text-steel-500">
                {t('period')} <DateDisplay value={q.data.from} /> – <DateDisplay value={q.data.to} /> ·{' '}
                {t('generated')} <DateDisplay value={q.data.generated_on} />
              </p>
            </div>
            <div className="text-right">
              <Link href={`/${locale}/partners/${id}`} className="text-section font-semibold underline print:no-underline">
                {q.data.partner_name}
              </Link>
              {q.data.partner_address && <p className="text-body text-steel-700">{q.data.partner_address}</p>}
              {q.data.partner_tax_number && (
                <p className="font-mono text-metadata text-steel-500">{q.data.partner_tax_number}</p>
              )}
            </div>
          </header>

          <section className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {q.data.totals.length === 0 && <p className="text-body text-steel-600">{t('nothing')}</p>}
            {q.data.totals.map((tot) => (
              <div key={tot.currency} className="rounded-lg border border-steel-200 p-3">
                <p className="text-metadata font-medium text-steel-500">{tot.currency}</p>
                <dl className="mt-1 grid grid-cols-2 gap-x-3 gap-y-0.5 text-body">
                  <dt className="text-steel-600">{t('invoiced')}</dt>
                  <dd className="text-right"><Money minor={tot.invoiced} currency={tot.currency as Currency} /></dd>
                  <dt className="text-steel-600">{t('received')}</dt>
                  <dd className="text-right"><Money minor={tot.received} currency={tot.currency as Currency} /></dd>
                  <dt className="font-medium">{t('outstanding')}</dt>
                  <dd className="text-right font-medium"><Money minor={tot.outstanding} currency={tot.currency as Currency} /></dd>
                  <dt className="text-steel-600">{t('overdue')}</dt>
                  <dd className="text-right"><Money minor={tot.overdue} currency={tot.currency as Currency} /></dd>
                </dl>
              </div>
            ))}
          </section>

          <section>
            <h2 className="text-section font-semibold">{t('invoices')}</h2>
            <div className="table-container mt-2">
              <table className="table">
                <thead>
                  <tr>
                    <th>{t('number')}</th>
                    <th>{t('order')}</th>
                    <th>{t('issued')}</th>
                    <th>{t('due')}</th>
                    <th className="text-right">{t('gross')}</th>
                    <th className="text-right">{t('paid')}</th>
                    <th className="text-right">{t('openAmount')}</th>
                  </tr>
                </thead>
                <tbody>
                  {q.data.lines.map((l) => {
                    const overdue = l.outstanding > 0 && l.payment_date && l.payment_date < q.data!.generated_on;
                    return (
                      <tr key={l.invoice_id}>
                        <td className="font-mono">
                          {l.number}
                          {l.kind === 'storno' && <span className="ml-1 text-metadata text-steel-500">{t('storno')}</span>}
                          {l.bucket === 'stornoed' && <span className="ml-1 text-metadata text-steel-500">{t('stornoed')}</span>}
                        </td>
                        <td className="font-mono text-metadata">{l.order_number}</td>
                        <td><DateDisplay value={l.issue_date} /></td>
                        <td className={overdue ? 'font-medium text-signal' : undefined}>
                          {l.payment_date ? <DateDisplay value={l.payment_date} /> : '—'}
                        </td>
                        <td className="text-right"><Money minor={l.gross_amount} currency={l.currency as Currency} /></td>
                        <td className="text-right"><Money minor={l.paid_amount} currency={l.currency as Currency} /></td>
                        <td className="text-right font-medium">
                          {l.outstanding > 0 ? <Money minor={l.outstanding} currency={l.currency as Currency} /> : '—'}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </section>

          {q.data.payments.length > 0 && (
            <section>
              <h2 className="text-section font-semibold">{t('payments')}</h2>
              <ul className="mt-2 divide-y divide-steel-200">
                {q.data.payments.map((p) => {
                  const line = q.data!.lines.find((l) => l.invoice_id === p.invoice_id);
                  return (
                    <li key={p.id} className="flex flex-wrap gap-x-4 py-1.5 text-body">
                      <DateDisplay value={p.paid_on} />
                      <span className="font-mono">{line?.number ?? `#${p.invoice_id}`}</span>
                      <Money minor={p.amount_minor} currency={(line?.currency ?? 'HUF') as Currency} />
                    </li>
                  );
                })}
              </ul>
            </section>
          )}

          <p className="text-metadata text-steel-500">{t('footer')}</p>
        </article>
      )}
    </AppShell>
  );
}
