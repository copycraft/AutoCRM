'use client';

import { useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { leadsApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { useAuth } from '@/lib/auth/context';
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
  const { user } = useAuth();
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
    onMutate: async () => {
      await qc.cancelQueries({ queryKey: qk.lead(leadId) });
      const prev = qc.getQueryData<LeadDetail>(qk.lead(leadId));
      const now = new Date().toISOString();
      const trimmedNote = note.trim() || null;
      if (prev && selected) {
        qc.setQueryData<LeadDetail>(qk.lead(leadId), {
          ...prev,
          stage: { stage_key: selected.stage_key, entered_at: now },
          history: [
            ...prev.history.map((h, i, all) =>
              i === all.length - 1 ? { ...h, left_at: now } : h,
            ),
            {
              id: -Date.now(),
              stage_key: selected.stage_key,
              label_hu: selected.label_hu,
              entered_at: now,
              left_at: null,
              entered_by: user?.id ?? null,
              entered_by_name: user?.display_name ?? null,
              note: trimmedNote,
            },
          ],
        });
      }
      return { prev };
    },
    onError: (e, _vars, context) => {
      if (context?.prev) qc.setQueryData(qk.lead(leadId), context.prev);
      setError(errorMessage(e, ter, ter('unknownError')));
    },
    onSettled: () => {
      void qc.invalidateQueries({ queryKey: qk.lead(leadId) });
      void qc.invalidateQueries({ queryKey: qk.leadTransitions(leadId) });
      void qc.invalidateQueries({ queryKey: ['leads'] });
    },
    onSuccess: () => {
      onClose();
    },
  });

  const valid = effective !== '' && (!needsNote || note.trim() !== '');

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
            <Dialog.Title className="text-section font-semibold">{t('stageChange')}</Dialog.Title>
            <Dialog.Description className="text-sm text-steel-500">
              {t('currentStage')}: {currentLabel}
            </Dialog.Description>
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
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
