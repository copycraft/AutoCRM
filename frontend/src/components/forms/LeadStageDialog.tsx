'use client';

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { leadsApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import type { LeadDetail, StageDefinition } from '@/lib/api/types';

export function LeadStageDialog({
  leadId,
  detail,
  definitions,
  onClose,
}: {
  leadId: number;
  detail: LeadDetail;
  definitions: StageDefinition[];
  onClose: () => void;
}) {
  const t = useTranslations('leads');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const current = detail.stage?.stage_key;
  const currentDef = definitions.find((d) => d.key === current);
  // Backend owns 'won': POSTing it returns 422 use_conversion — filter it
  // client-side with an explanation instead of letting users hit the error.
  const targets = definitions.filter((d) => d.key !== current && d.key !== 'won' && d.is_active);
  const [target, setTarget] = useState(targets[0]?.key ?? '');
  const [note, setNote] = useState('');
  const [error, setError] = useState<string | null>(null);

  const targetDef = definitions.find((d) => d.key === target);
  const needsNote =
    !!targetDef &&
    !!currentDef &&
    (targetDef.position < currentDef.position || currentDef.is_terminal);

  const change = useMutation({
    mutationFn: () => leadsApi.stage(leadId, { stage: target, note: note.trim() || undefined }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: qk.lead(leadId) });
      void qc.invalidateQueries({ queryKey: ['leads'] });
      onClose();
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  const valid = target !== '' && (!needsNote || note.trim() !== '');

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-steel-900/40 p-4" role="dialog" aria-modal="true" aria-label={t('stageChange')} onClick={onClose}>
      <div className="card w-full max-w-md" onClick={(e) => e.stopPropagation()}>
        <div className="card-header">
          <h2 className="text-section font-semibold">{t('stageChange')}</h2>
          <p className="text-sm text-steel-500">
            {t('currentStage')}: {currentDef?.label_hu ?? current ?? '—'}
          </p>
        </div>
        <div className="card-content space-y-4">
          {targets.length === 0 ? (
            <p className="text-sm text-steel-500">{t('noOtherStage')}</p>
          ) : (
            <>
              <div>
                <label className="label" htmlFor="ls-target">{t('targetStage')}</label>
                <select id="ls-target" className="input" value={target} onChange={(e) => setTarget(e.target.value)}>
                  {targets.map((d) => (
                    <option key={d.key} value={d.key}>
                      {d.label_hu}
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
          <button className="btn-primary" disabled={!valid || change.isPending || targets.length === 0} onClick={() => change.mutate()}>
            {change.isPending ? tc('saving') : tc('save')}
          </button>
        </div>
      </div>
    </div>
  );
}
