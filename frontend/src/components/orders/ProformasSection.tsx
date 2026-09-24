'use client';

// Proformas (díjbekérő) on the order screen.
//
// Its own section, on purpose. A proforma is a request for payment: it is reported
// nowhere, grants no VAT deduction right, and has no status, no NAV transaction and no
// chain — so mixing it into the invoice list would be a lie told in layout. It is also
// available from the day the order is priced, long before any invoice exists, which is
// most of what it is for.

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { invoicesApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { canEditOrders, useAuth } from '@/lib/auth/context';
import { Money } from '@/components/ui/Money';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { DocumentLink } from '@/components/orders/InvoicesSection';
import type { Currency } from '@/lib/api/types';

export function ProformasSection({
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

  const [open, setOpen] = useState(false);
  const [note, setNote] = useState('');
  const [paymentDate, setPaymentDate] = useState('');
  const [error, setError] = useState<string | null>(null);

  const proformas = useQuery({
    queryKey: qk.proformas(orderId),
    queryFn: () => invoicesApi.proformas(orderId),
  });

  const create = useMutation({
    mutationFn: () =>
      invoicesApi.createProforma(orderId, {
        ...(note.trim() ? { note: note.trim() } : {}),
        ...(paymentDate ? { payment_date: paymentDate } : {}),
      }),
    onSuccess: () => {
      setOpen(false);
      setNote('');
      setPaymentDate('');
      setError(null);
      void qc.invalidateQueries({ queryKey: qk.proformas(orderId) });
      void qc.invalidateQueries({ queryKey: qk.documents(orderId) });
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  const items = proformas.data?.items ?? [];

  return (
    <section className="border-t border-steel-200 pt-5">
      <div className="flex items-center justify-between">
        <h2 className="text-section font-semibold">{t('proformaTitle')}</h2>
        {canIssue && (
          <button className="btn-secondary btn-sm" onClick={() => setOpen(true)}>
            {t('proformaCreate')}
          </button>
        )}
      </div>
      <p className="mt-1 text-metadata text-steel-500">{t('proformaExplainer')}</p>

      {error && (
        <p className="mt-3 text-body text-signal" role="alert">
          {error}
        </p>
      )}

      {proformas.isLoading ? (
        <p className="mt-3 text-body text-steel-500">{tc('loading')}</p>
      ) : items.length === 0 ? (
        <p className="mt-3 text-body text-steel-500">{t('proformaNone')}</p>
      ) : (
        <ul className="mt-3 divide-y divide-steel-200">
          {items.map((proforma) => (
            <li key={proforma.id} className="flex flex-wrap items-center gap-x-3 gap-y-1 py-2">
              <span className="font-mono text-body font-medium">{proforma.number}</span>
              <DateDisplay
                value={proforma.issue_date}
                className="text-metadata text-steel-500"
              />
              {proforma.payment_date && (
                <span className="text-metadata text-steel-500">
                  {t('paymentDate')}: <DateDisplay value={proforma.payment_date} />
                </span>
              )}
              <DocumentLink documentId={proforma.document_id} label={t('pdf')} />
              <Money minor={proforma.gross_amount} currency={currency} className="ml-auto" />
            </li>
          ))}
        </ul>
      )}

      {open && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-steel-900/40 p-4">
          <div
            className="card w-full max-w-md"
            role="dialog"
            aria-modal="true"
            aria-label={t('proformaCreate')}
          >
            <div className="card-header">
              <h3 className="text-section font-semibold">{t('proformaCreate')}</h3>
            </div>
            <div className="card-content space-y-3">
              <p className="text-body text-steel-500">{t('proformaDialogBody')}</p>
              <div>
                <label className="label" htmlFor="proforma-payment-date">
                  {t('paymentDate')}
                </label>
                <input
                  id="proforma-payment-date"
                  type="date"
                  className="input"
                  value={paymentDate}
                  onChange={(e) => setPaymentDate(e.target.value)}
                />
              </div>
              <div>
                <label className="label" htmlFor="proforma-note">
                  {t('proformaNote')}
                </label>
                <textarea
                  id="proforma-note"
                  className="input"
                  rows={2}
                  value={note}
                  onChange={(e) => setNote(e.target.value)}
                />
              </div>
            </div>
            <div className="card-footer justify-end">
              <button
                className="btn-ghost"
                onClick={() => setOpen(false)}
                disabled={create.isPending}
              >
                {tc('cancel')}
              </button>
              <button
                className="btn-primary"
                disabled={create.isPending}
                onClick={() => create.mutate()}
              >
                {create.isPending ? tc('processing') : t('proformaCreate')}
              </button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
