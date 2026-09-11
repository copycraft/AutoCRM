'use client';

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { ordersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { ErrorState } from '@/components/ui/ErrorState';
import type { OrderDetail, StageDefinition, TransitionOption } from '@/lib/api/types';

export function OrderStageDialog({
  orderId,
  detail,
  definitions,
  onClose,
}: {
  orderId: number;
  detail: OrderDetail;
  /** Stage definitions: labels and gate details only. Transition rules come from the backend. */
  definitions: StageDefinition[];
  onClose: () => void;
}) {
  const t = useTranslations('orders');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const ti = useTranslations('images');
  const qc = useQueryClient();
  const [target, setTarget] = useState('');
  const [note, setNote] = useState('');
  const [error, setError] = useState<string | null>(null);

  const transitions = useQuery({
    queryKey: qk.orderTransitions(orderId),
    queryFn: () => ordersApi.transitions(orderId),
  });
  const options = transitions.data?.items.filter((o) => o.manual) ?? [];
  const selected: TransitionOption | undefined =
    options.find((o) => o.stage_key === target) ?? options[0];
  const effective = selected?.stage_key ?? '';
  const defOf = (key: string) => definitions.find((d) => d.key === key);
  const selectedDef = effective ? defOf(effective) : undefined;
  const needsNote = selected?.requires_note ?? false;
  const gateBlocked = selected ? !selected.gates_met : false;

  const change = useMutation({
    mutationFn: () =>
      ordersApi.stage(orderId, { stage: effective, note: note.trim() || undefined }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: qk.order(orderId) });
      void qc.invalidateQueries({ queryKey: qk.orderStages(orderId) });
      void qc.invalidateQueries({ queryKey: qk.orderTransitions(orderId) });
      void qc.invalidateQueries({ queryKey: ['orders'] });
      onClose();
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  const valid = effective !== '' && (!needsNote || note.trim() !== '') && !gateBlocked;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-steel-900/40 p-4" role="dialog" aria-modal="true" aria-label={t('changeStage')} onClick={onClose}>
      <div className="card w-full max-w-md" onClick={(e) => e.stopPropagation()}>
        <div className="card-header">
          <h2 className="text-section font-semibold">{t('changeStage')}</h2>
          <p className="text-sm text-steel-500">
            {t('currentStage')}: {detail.stage.label_hu} · {t('daysInStage', { days: detail.stage.days_in_stage })}
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
                <label className="label" htmlFor="os-target">{t('targetStage')}</label>
                <select id="os-target" className="input" value={effective} onChange={(e) => setTarget(e.target.value)}>
                  {options.map((o) => {
                    const d = defOf(o.stage_key);
                    return (
                      <option key={o.stage_key} value={o.stage_key} disabled={!o.gates_met}>
                        {o.label_hu}
                        {d?.is_exit ? ` ${t('exitSuffix')}` : ''}
                        {d?.is_terminal ? ` ${t('terminalSuffix')}` : ''}
                        {!o.gates_met ? ` (${t('gateBlocked')})` : ''}
                      </option>
                    );
                  })}
                </select>
                {selectedDef && selectedDef.min_images > 0 && selectedDef.required_image_category && (
                  <p className="mt-1 text-xs text-steel-900">
                    {t('gateRequires')}:{' '}
                    {t('gateRequirement', {
                      count: selectedDef.min_images,
                      category: ti(selectedDef.required_image_category),
                    })}
                  </p>
                )}
              </div>
              <div>
                <label className="label" htmlFor="os-note">
                  {t('note')}{needsNote ? ' *' : ''}
                </label>
                <textarea id="os-note" rows={3} className="input" value={note} onChange={(e) => setNote(e.target.value)} />
                <p className="mt-1 text-xs text-steel-500">{t('noteHint')}</p>
              </div>
            </>
          )}
          <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-xs text-steel-900">{t('gateHint')}</p>
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
