'use client';

import { useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
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
  const ter = useTranslations('errors');
  const tof = useTranslations('orders');
  const locale = useLocale();
  const router = useRouter();
  const qc = useQueryClient();
  const lead = detail.lead;

  const [title, setTitle] = useState(lead.title);
  // Explicit user choice; null means "follow the partner default".
  const [currencyOverride, setCurrencyOverride] = useState<Currency | null>(null);
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

  // Prefilled from the lead's partner default, but always changeable —
  // an EUR job for a HUF-default partner must be possible.
  const partnerDefault: Currency = partnerQuery.data?.partner.default_currency ?? 'HUF';
  const effectiveCurrency = currencyOverride ?? partnerDefault;

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
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  const canConvert = title.trim() !== '' && partnerId !== undefined && !convert.isPending;

  return (
    <Dialog.Root
      open
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-steel-900/40" />
        <Dialog.Content className="card fixed left-1/2 top-1/2 z-50 max-h-[85vh] w-[90vw] max-w-md -translate-x-1/2 -translate-y-1/2 overflow-y-auto">
          <div className="card-header">
            <Dialog.Title className="text-section font-semibold">{t('convertTitle')}</Dialog.Title>
            <Dialog.Description className="text-metadata text-steel-500">{t('wonOnlyViaConvert')}</Dialog.Description>
          </div>
          <div className="card-content space-y-4">
            <div>
              <label className="label" htmlFor="lc-title">{t('title')} *</label>
              <input id="lc-title" className="input" value={title} onChange={(e) => setTitle(e.target.value)} />
            </div>
            <div>
              <span className="label">{t('convertPartner')}</span>
              <p className="rounded-lg border border-steel-200 bg-panel px-3 py-2 text-body">
                {partnerQuery.data?.partner.name ?? '—'}
              </p>
              {!partnerId && (
                <p className="mt-1 text-metadata text-steel-900">{t('noPartnerForConvert')}</p>
              )}
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="label" htmlFor="lc-currency">{t('convertCurrency')}</label>
                <select
                  id="lc-currency"
                  className="input"
                  value={effectiveCurrency}
                  onChange={(e) => setCurrencyOverride(toCurrency(e.target.value))}
                >
                  <option value="HUF">HUF</option>
                  <option value="EUR">EUR</option>
                </select>
                {partnerQuery.data && currencyOverride === null && (
                  <p className="mt-1 text-metadata text-steel-500">{t('partnerCurrencyHint')}</p>
                )}
              </div>
            <div>
              <label className="label" htmlFor="lc-pt">{tof('projectType')}</label>
              <select id="lc-pt" className="input" value={projectTypeId} disabled={projectTypes.isLoading} onChange={(e) => setProjectTypeId(e.target.value)}>
                <option value="">—</option>
                {(projectTypes.data?.items ?? [])
                  .filter((p) => p.is_active)
                  .map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.label_hu}
                    </option>
                  ))}
              </select>
              {projectTypes.isError && (
                <p className="mt-1 text-metadata text-steel-900" role="alert">
                  {errorMessage(projectTypes.error, ter, ter('unknownError'))}
                </p>
              )}
            </div>
            </div>
            <div>
              <label className="label" htmlFor="lc-desc">{t('description')}</label>
              <textarea id="lc-desc" rows={3} className="input" value={description} onChange={(e) => setDescription(e.target.value)} />
            </div>
            {error && (
              <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">{error}</p>
            )}
          </div>
          <div className="card-footer justify-end">
            <button className="btn-ghost" onClick={onClose}>{tc('cancel')}</button>
            <button className="btn-primary" disabled={!canConvert} onClick={() => convert.mutate()}>
              {convert.isPending ? t('converting') : t('convert')}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
