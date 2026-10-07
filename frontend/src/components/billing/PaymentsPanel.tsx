'use client';

// Payments received on one invoice (0049): a customer may pay in parts. Each payment is
// booked with its day and method; the invoice counts as paid once they add up to the gross.

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Trash2 } from 'lucide-react';
import { paymentsApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { minorToMajorString, parseMajorToMinor } from '@/lib/utils/format';
import { Money } from '@/components/ui/Money';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { LoadingState } from '@/components/ui/LoadingState';
import type { Currency } from '@/lib/api/types';

const METHODS = ['TRANSFER', 'CASH', 'CARD', 'OTHER'] as const;

function todayIso(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

export function PaymentsPanel({
  invoiceId,
  gross,
  paid,
  currency,
  canEdit,
}: {
  invoiceId: number;
  gross: number;
  paid: number;
  currency: Currency;
  canEdit: boolean;
}) {
  const t = useTranslations('payments');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const open = Math.max(gross - paid, 0);
  const [amount, setAmount] = useState(minorToMajorString(open));
  const [day, setDay] = useState(todayIso());
  const [method, setMethod] = useState<(typeof METHODS)[number]>('TRANSFER');
  const [note, setNote] = useState('');
  const [error, setError] = useState<string | null>(null);

  const key = ['invoices', invoiceId, 'payments'];
  const list = useQuery({ queryKey: key, queryFn: () => paymentsApi.list(invoiceId) });
  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ['invoices'] });
    void qc.invalidateQueries({ queryKey: ['orders'] });
  };
  const add = useMutation({
    mutationFn: () => {
      const minor = parseMajorToMinor(amount);
      if (minor === null || minor <= 0) throw new Error(t('badAmount'));
      return paymentsApi.add(invoiceId, { amount_minor: minor, paid_on: day, method, note: note || null });
    },
    onSuccess: (data) => {
      setError(null);
      setNote('');
      qc.setQueryData(key, data);
      refresh();
    },
    onError: (e) => setError(e instanceof Error && !('code' in e) ? e.message : errorMessage(e, ter, ter('unknownError'))),
  });
  const remove = useMutation({
    mutationFn: (paymentId: number) => paymentsApi.remove(invoiceId, paymentId),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: key });
      refresh();
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  const items = list.data?.items ?? [];
  return (
    <div className="mt-3 rounded-lg border border-steel-200 bg-panel p-3">
      <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1 text-metadata text-steel-600">
        <span>
          {t('received')} <Money minor={paid} currency={currency} />
        </span>
        <span>
          {t('open')} <Money minor={open} currency={currency} />
        </span>
      </div>
      {list.isLoading ? (
        <LoadingState />
      ) : items.length === 0 ? (
        <p className="mt-2 text-metadata text-steel-500">{t('none')}</p>
      ) : (
        <ul className="mt-2 divide-y divide-steel-200">
          {items.map((p) => (
            <li key={p.id} className="flex flex-wrap items-center gap-x-3 py-1.5 text-body">
              <DateDisplay value={p.paid_on} />
              <Money minor={p.amount_minor} currency={currency} />
              <span className="text-metadata text-steel-500">{t(`methods.${p.method}`)}</span>
              {p.note && <span className="text-metadata text-steel-600">{p.note}</span>}
              {p.created_by_name && <span className="text-metadata text-steel-400">{p.created_by_name}</span>}
              {canEdit && (
                <button
                  className="btn-ghost btn-sm ml-auto"
                  aria-label={t('remove')}
                  disabled={remove.isPending}
                  onClick={() => {
                    if (window.confirm(t('removeConfirm'))) remove.mutate(p.id);
                  }}
                >
                  <Trash2 className="h-4 w-4" aria-hidden />
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
      {canEdit && open > 0 && (
        <div className="mt-3 flex flex-wrap items-end gap-2">
          <div>
            <label className="label" htmlFor={`pay-amount-${invoiceId}`}>{t('amount', { currency })}</label>
            <input
              id={`pay-amount-${invoiceId}`}
              className="input w-36 text-right font-mono"
              inputMode="decimal"
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
            />
          </div>
          <div>
            <label className="label" htmlFor={`pay-day-${invoiceId}`}>{t('day')}</label>
            <input
              id={`pay-day-${invoiceId}`}
              type="date"
              className="input w-40"
              value={day}
              max={todayIso()}
              onChange={(e) => setDay(e.target.value)}
            />
          </div>
          <div>
            <label className="label" htmlFor={`pay-method-${invoiceId}`}>{t('method')}</label>
            <select
              id={`pay-method-${invoiceId}`}
              className="input w-36"
              value={method}
              onChange={(e) => setMethod(e.target.value as (typeof METHODS)[number])}
            >
              {METHODS.map((m) => (
                <option key={m} value={m}>
                  {t(`methods.${m}`)}
                </option>
              ))}
            </select>
          </div>
          <div className="min-w-[10rem] flex-1">
            <label className="label" htmlFor={`pay-note-${invoiceId}`}>{t('note')}</label>
            <input
              id={`pay-note-${invoiceId}`}
              className="input"
              maxLength={500}
              value={note}
              onChange={(e) => setNote(e.target.value)}
            />
          </div>
          <button className="btn-primary" disabled={add.isPending} onClick={() => add.mutate()}>
            {t('book')}
          </button>
        </div>
      )}
      {error && (
        <p className="mt-2 rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
