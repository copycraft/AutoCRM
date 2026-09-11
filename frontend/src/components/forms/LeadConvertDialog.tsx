'use client';

import { useState } from 'react';
import { useRouter } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { skipToken, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { configApi, leadsApi, partnersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import type { Currency, LeadDetail } from '@/lib/api/types';

function toCurrency(value: string): Currency {
  return value === 'EUR' ? 'EUR' : 'HUF';
}

export function LeadConvertDialog({
  leadId,
  detail,
  onClose,
}: {
  leadId: number;
  detail: LeadDetail;
  onClose: () => void;
}) {
  const t = useTranslations('leads');
  const tc = useTranslations('common');
  const locale = useLocale();
  const router = useRouter();
  const qc = useQueryClient();
  const lead = detail.lead;

  const [title, setTitle] = useState(lead.title);
  const [currency, setCurrency] = useState<Currency>('HUF');
  const [projectTypeId, setProjectTypeId] = useState('');
  const [description, setDescription] = useState(lead.description ?? '');
  const [error, setError] = useState<string | null>(null);

  const partnerId = lead.partner_id ?? undefined;
  const partnerQuery = useQuery({
    queryKey: qk.partner(partnerId ?? 0),
    queryFn: partnerId === undefined ? skipToken : () => partnersApi.get(partnerId),
  });
  const projectTypes = useQuery({
    queryKey: qk.projectTypes,
    queryFn: () => configApi.projectTypes(),
  });

  // Default currency from the lead's partner; falls back to HUF.
  const defaultCurrency: Currency = partnerQuery.data?.partner.default_currency ?? 'HUF';
  const effectiveCurrency = partnerQuery.data ? defaultCurrency : currency;

  const convert = useMutation({
    mutationFn: () =>
      leadsApi.convert(leadId, {
        title: title.trim() || undefined,
        partner_id: partnerId,
        currency: effectiveCurrency,
        project_type_id: projectTypeId ? Number(projectTypeId) : undefined,
        description: description.trim() || undefined,
        items: [],
      }),
    onSuccess: (order) => {
      void qc.invalidateQueries({ queryKey: qk.lead(leadId) });
      void qc.invalidateQueries({ queryKey: ['leads'] });
      void qc.invalidateQueries({ queryKey: ['orders'] });
      router.push(`/${locale}/orders/${order.id}`);
    },
    onError: (e) => setError(errorMessage(e, 'Ismeretlen hiba.')),
  });

  const canConvert = title.trim() !== '' && partnerId !== undefined && !convert.isPending;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-steel-900/40 p-4" role="dialog" aria-modal="true" aria-label={t('convertTitle')} onClick={onClose}>
      <div className="card w-full max-w-md" onClick={(e) => e.stopPropagation()}>
        <div className="card-header">
          <h2 className="text-section font-semibold">{t('convertTitle')}</h2>
          <p className="text-sm text-steel-500">{t('wonOnlyViaConvert')}</p>
        </div>
        <div className="card-content space-y-4">
          <div>
            <label className="label" htmlFor="lc-title">{t('title')} *</label>
            <input id="lc-title" className="input" value={title} onChange={(e) => setTitle(e.target.value)} />
          </div>
          <div>
            <span className="label">{t('convertPartner')}</span>
            <p className="rounded-lg border border-steel-200 bg-panel px-3 py-2 text-sm">
              {partnerQuery.data?.partner.name ?? '—'}
            </p>
            {!partnerId && (
              <p className="mt-1 text-xs text-steel-900">{t('noPartnerForConvert')}</p>
            )}
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="label" htmlFor="lc-currency">{t('convertCurrency')}</label>
              {partnerQuery.data ? (
                <p className="rounded-lg border border-steel-200 bg-panel px-3 py-2 font-mono text-sm">
                  {defaultCurrency} <span className="text-steel-500">(partner)</span>
                </p>
              ) : (
                <select id="lc-currency" className="input" value={currency} onChange={(e) => setCurrency(toCurrency(e.target.value))}>
                  <option value="HUF">HUF</option>
                  <option value="EUR">EUR</option>
                </select>
              )}
            </div>
            <div>
              <label className="label" htmlFor="lc-pt">Projekttípus</label>
              <select id="lc-pt" className="input" value={projectTypeId} onChange={(e) => setProjectTypeId(e.target.value)}>
                <option value="">—</option>
                {(projectTypes.data?.items ?? [])
                  .filter((p) => p.is_active)
                  .map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.label_hu}
                    </option>
                  ))}
              </select>
            </div>
          </div>
          <div>
            <label className="label" htmlFor="lc-desc">{t('description')}</label>
            <textarea id="lc-desc" rows={3} className="input" value={description} onChange={(e) => setDescription(e.target.value)} />
          </div>
          {error && (
            <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-sm text-steel-900" role="alert">{error}</p>
          )}
        </div>
        <div className="card-footer justify-end">
          <button className="btn-ghost" onClick={onClose}>{tc('cancel')}</button>
          <button className="btn-primary" disabled={!canConvert} onClick={() => convert.mutate()}>
            {convert.isPending ? 'Átalakítás…' : t('convert')}
          </button>
        </div>
      </div>
    </div>
  );
}
