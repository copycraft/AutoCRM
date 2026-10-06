'use client';

// Two nudges on the home screen: the signed-in salesperson's quotes about to run out
// (the same list the daily notification is built from), and invoices the customer has
// not paid past the deadline, with how many reminders already went out.

import Link from 'next/link';
import { useState } from 'react';
import { useLocale, useTranslations } from 'next-intl';
import { useQuery } from '@tanstack/react-query';
import { CollapsibleSection } from '@/components/ui/CollapsibleSection';
import { ErrorState } from '@/components/ui/ErrorState';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { Money } from '@/components/ui/Money';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { invoicesApi, leadsApi } from '@/lib/api/endpoints';
import { useAuth } from '@/lib/auth/context';
import type { Currency } from '@/lib/api/types';

export function DashboardReminders() {
  const t = useTranslations('dashboard');
  const locale = useLocale();
  const { user } = useAuth();
  const [mineOnly, setMineOnly] = useState(true);
  const canBill = user?.role === 'admin' || user?.role === 'office';

  const quotes = useQuery({
    queryKey: ['expiring-quotes', mineOnly ? user?.id : 'all'],
    queryFn: () => leadsApi.expiringQuotes({ days: 7, assigned_to: mineOnly ? user?.id : undefined }),
    enabled: !!user,
    staleTime: 60_000,
  });
  const overdue = useQuery({
    queryKey: ['overdue-invoices'],
    queryFn: () => invoicesApi.overdue(),
    enabled: canBill,
    staleTime: 60_000,
  });
  const quoteRows = quotes.data?.items ?? [];
  const overdueRows = overdue.data?.items ?? [];

  return (
    <>
      <CollapsibleSection
        storageKey="dash-expiring-quotes"
        title={t('expiringQuotesTitle')}
        count={quotes.isPending ? undefined : quoteRows.length}
      >
        <label className="mb-2 flex items-center gap-2 text-metadata text-steel-500">
          <input
            type="checkbox"
            className="rounded border-steel-200 accent-steel-900"
            checked={mineOnly}
            onChange={(e) => setMineOnly(e.target.checked)}
          />
          {t('mineOnly')}
        </label>
        {quotes.error ? (
          <ErrorState error={quotes.error} onRetry={() => void quotes.refetch()} />
        ) : quoteRows.length === 0 ? (
          <p className="text-body text-steel-500">{quotes.isPending ? '…' : t('expiringQuotesEmpty')}</p>
        ) : (
          <ul className="space-y-2" data-testid="expiring-quotes">
            {quoteRows.map((q) => (
              <li key={q.lead_id}>
                <Link
                  href={`/${locale}/leads/${q.lead_id}`}
                  className="card flex flex-wrap items-center gap-x-4 gap-y-1 p-4 hover:border-steel-900"
                >
                  <StatusBadge tone={q.days_left <= 2 ? 'signal' : 'steel'}>
                    {q.days_left < 0
                      ? t('expiredDaysAgo', { days: -q.days_left })
                      : q.days_left === 0
                        ? t('expiresToday')
                        : t('expiresInDays', { days: q.days_left })}
                  </StatusBadge>
                  <span className="min-w-0 flex-1">
                    <span className="block text-body font-medium">{q.title}</span>
                    <span className="block text-metadata text-steel-500">
                      {q.partner_name ?? q.contact_name ?? '—'} · <DateDisplay value={q.valid_until} />
                      {!mineOnly && ` · ${q.assigned_name ?? t('unassigned')}`}
                    </span>
                  </span>
                  {q.quoted_value_minor != null && q.currency && (
                    <Money minor={q.quoted_value_minor} currency={q.currency as Currency} />
                  )}
                </Link>
              </li>
            ))}
          </ul>
        )}
      </CollapsibleSection>

      {canBill && (
        <CollapsibleSection
          storageKey="dash-overdue-invoices"
          title={t('overdueInvoicesTitle')}
          count={overdue.isPending ? undefined : overdueRows.length}
        >
          {overdue.error ? (
            <ErrorState error={overdue.error} onRetry={() => void overdue.refetch()} />
          ) : overdueRows.length === 0 ? (
            <p className="text-body text-steel-500">{overdue.isPending ? '…' : t('overdueInvoicesEmpty')}</p>
          ) : (
            <ul className="space-y-2" data-testid="overdue-invoices">
              {overdueRows.slice(0, 20).map((i) => (
                <li key={i.id}>
                  <Link
                    href={`/${locale}/orders/${i.order_id}`}
                    className="card flex flex-wrap items-center gap-x-4 gap-y-1 p-4 hover:border-steel-900"
                  >
                    <StatusBadge tone="signal">{t('overdueDays', { days: i.days_overdue })}</StatusBadge>
                    <span className="min-w-0 flex-1">
                      <span className="block text-body font-medium">
                        {i.number} · {i.partner_name}
                      </span>
                      <span className="block text-metadata text-steel-500">
                        {t('paymentDue')}: <DateDisplay value={i.payment_date} /> ·{' '}
                        {t('remindersSent', { count: i.reminders_sent })}
                      </span>
                    </span>
                    <Money minor={i.gross_amount} currency={i.currency as Currency} />
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </CollapsibleSection>
      )}
    </>
  );
}
