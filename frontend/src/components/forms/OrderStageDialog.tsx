'use client';

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ordersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import type { OrderDetail, StageDefinition } from '@/lib/api/types';



export function OrderStageDialog({
  orderId,
  detail,
  definitions,
  onClose,
}: {
  orderId: number;
  detail: OrderDetail;
  definitions: StageDefinition[];
  onClose: () => void;
}) {
  const t = useTranslations('orders');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const ti = useTranslations('images');
  const qc = useQueryClient();
  const current = detail.stage.key;
  const currentDef = definitions.find((d) => d.key === current);
  const targets = definitions.filter((d) => d.key !== current && d.is_active);
  const [target, setTarget] = useState(targets[0]?.key ?? '');
  const [note, setNote] = useState('');
  const [error, setError] = useState<string | null>(null);

  const targetDef = definitions.find((d) => d.key === target);
  const needsNote =
    !!targetDef &&
    !!currentDef &&
    (targetDef.position < currentDef.position || currentDef.is_terminal);

  const change = useMutation({
    mutationFn: () => ordersApi.stage(orderId, { stage: target, note: note.trim() || undefined }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: qk.order(orderId) });
      void qc.invalidateQueries({ queryKey: qk.orderStages(orderId) });
      void qc.invalidateQueries({ queryKey: ['orders'] });
      onClose();
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  const valid = target !== '' && (!needsNote || note.trim() !== '');

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-steel-900/40 p-4" role="dialog" aria-modal="true" aria-label={t('changeStage')} onClick={onClose}>
      <div className="card w-full max-w-md" onClick={(e) => e.stopPropagation()}>
        <div className="card-header">
          <h2 className="text-section font-semibold">{t('changeStage')}</h2>
          <p className="text-sm text-steel-500">
            {t('currentStage')}: {detail.stage.label_hu} · {detail.stage.days_in_stage} napja
          </p>
        </div>
        <div className="card-content space-y-4">
          {targets.length === 0 ? (
            <p className="text-sm text-steel-500">{t('noOtherStage')}</p>
          ) : (
            <>
              <div>
                <label className="label" htmlFor="os-target">{t('targetStage')}</label>
                <select id="os-target" className="input" value={target} onChange={(e) => setTarget(e.target.value)}>
                  {targets.map((d) => (
                    <option key={d.key} value={d.key}>
                      {d.label_hu}
                      {d.is_exit ? ` ${t('exitSuffix')}` : ''}
                      {d.is_terminal ? ` ${t('terminalSuffix')}` : ''}
                    </option>
                  ))}
                </select>
                {targetDef && targetDef.min_images > 0 && targetDef.required_image_category && (
                  <p className="mt-1 text-xs text-steel-900">
                    {t('gateRequires')}:{' '}
                    {t('gateRequirement', {
                      count: targetDef.min_images,
                      category: ti(targetDef.required_image_category),
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
          <button className="btn-primary" disabled={!valid || change.isPending || targets.length === 0} onClick={() => change.mutate()}>
            {change.isPending ? tc('saving') : tc('save')}
          </button>
        </div>
      </div>
    </div>
  );
}
