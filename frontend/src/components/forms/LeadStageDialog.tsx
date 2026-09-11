'use client';

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { leadsApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { ErrorState } from '@/components/ui/ErrorState';
import type { LeadDetail, TransitionOption } from '@/lib/api/types';

export function LeadStageDialog({
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
  const qc = useQueryClient();
  const [target, setTarget] = useState('');
  const [note, setNote] = useState('');
  const [error, setError] = useState<string | null>(null);

  const transitions = useQuery({
    queryKey: qk.leadTransitions(leadId),
    queryFn: () => leadsApi.transitions(leadId),
  });
  // Non-manual targets (lead `won`: conversion only) are not offered; the
  // note below explains why. Which targets are manual comes from the backend.
  const options = transitions.data?.items.filter((o) => o.manual) ?? [];
  const selected: TransitionOption | undefined =
    options.find((o) => o.stage_key === target) ?? options[0];
  const effective = selected?.stage_key ?? '';
  const needsNote = selected?.requires_note ?? false;
  const currentLabel = detail.history.at(-1)?.label_hu ?? detail.stage?.stage_key ?? '—';

  const change = useMutation({
    mutationFn: () =>
      leadsApi.stage(leadId, { stage: effective, note: note.trim() || undefined }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: qk.lead(leadId) });
      void qc.invalidateQueries({ queryKey: qk.leadTransitions(leadId) });
      void qc.invalidateQueries({ queryKey: ['leads'] });
      onClose();
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  const valid = effective !== '' && (!needsNote || note.trim() !== '');

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-steel-900/40 p-4" role="dialog" aria-modal="true" aria-label={t('stageChange')} onClick={onClose}>
      <div className="card w-full max-w-md" onClick={(e) => e.stopPropagation()}>
        <div className="card-header">
          <h2 className="text-section font-semibold">{t('stageChange')}</h2>
          <p className="text-sm text-steel-500">
            {t('currentStage')}: {currentLabel}
          </p>
        </div>
        <div className="card-content space-y-4">
          {transitions.isLoading ? (
            <p className="text-sm text-steel-500">{tc('loading')}</p>
          ) : transitions.isError ? (
            <ErrorState error={transitions.error} onRetry={() => void transitions.refetch()} />
          ) : options.length === 0 ? (
            <p className="text-sm text-steel-500">{t('noOtherStage')}</p>
          ) : (
            <>
              <div>
                <label className="label" htmlFor="ls-target">{t('targetStage')}</label>
                <select id="ls-target" className="input" value={effective} onChange={(e) => setTarget(e.target.value)}>
                  {options.map((o) => (
                    <option key={o.stage_key} value={o.stage_key}>
                      {o.label_hu}
                    </option>
                  ))}
                </select>
              </div>
              <div>
                <label className="label" htmlFor="ls-note">
                  {t('note')}{needsNote ? ' *' : ''}
                </label>
                <textarea id="ls-note" rows={3} className="input" value={note} onChange={(e) => setNote(e.target.value)} />
                <p className="mt-1 text-xs text-steel-500">{t('noteHint')}</p>
              </div>
            </>
          )}
          <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-xs text-steel-900">{t('wonOnlyViaConvert')}</p>
          {error && (
            <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-sm text-steel-900" role="alert">{error}</p>
          )}
        </div>
        <div className="card-footer justify-end">
          <button className="btn-ghost" onClick={onClose}>{tc('cancel')}</button>
          <button className="btn-primary" disabled={!valid || change.isPending || options.length === 0} onClick={() => change.mutate()}>
            {change.isPending ? tc('saving') : tc('save')}
          </button>
        </div>
      </div>
    </div>
  );
}
