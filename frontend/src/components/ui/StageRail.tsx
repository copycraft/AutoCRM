'use client';

// StageRail — signature traveller component (FRONTEND_PLAN.md §12).
// Renders BACKEND stage definitions + history. Never hardcodes stages.

import { Check, Circle } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { cn, formatDateTime } from '@/lib/utils/format';
import type { StageDefinition, StageEntry } from '@/lib/api/types';

export function StageRail({
  stages,
  currentKey,
  daysInStage,
  openBlockers,
}: {
  stages: StageDefinition[];
  currentKey: string;
  daysInStage: number;
  openBlockers: number;
}) {
  const t = useTranslations('stageRail');
  const ordered = [...stages].sort((a, b) => a.position - b.position);
  const currentIdx = ordered.findIndex((s) => s.key === currentKey);

  return (
    <ol className="space-y-0" aria-label={t('title')}>
      {ordered.map((s, i) => {
        const done = currentIdx >= 0 && i < currentIdx;
        const current = s.key === currentKey;
        return (
          <li key={s.key} className="relative flex gap-3 pb-5 last:pb-0">
            {i < ordered.length - 1 && (
              <span
                className={cn('absolute left-[9px] top-5 h-full w-px', done ? 'bg-done' : 'bg-steel-200')}
                aria-hidden
              />
            )}
            <span className="mt-0.5 shrink-0">
              {done ? (
                <Check className="h-[19px] w-[19px] text-done" aria-label={t('done')} />
              ) : current ? (
                <span className="flex h-[19px] w-[19px] items-center justify-center" role="img" aria-label={t('current')}>
                  <span className="h-2.5 w-2.5 rounded-full bg-steel-900" aria-hidden />
                </span>
              ) : (
                <Circle className="h-[19px] w-[19px] text-steel-200" aria-hidden />
              )}
            </span>
            <div className="min-w-0">
              <p className={cn('text-sm', current ? 'font-semibold' : 'font-medium')}>{s.label_hu}</p>
              {current && (
                <p className="text-metadata text-steel-500">
                  {t('daysInStage', { days: daysInStage })}
                  {openBlockers > 0 && ` · ${t('openBlockers', { count: openBlockers })}`}
                </p>
              )}
            </div>
          </li>
        );
      })}
    </ol>
  );
}

export function StageHistoryList({ history }: { history: StageEntry[] }) {
  const t = useTranslations('stageRail');
  if (history.length === 0) return <p className="text-sm text-steel-500">{t('noHistory')}</p>;
  return (
    <ul className="space-y-3">
      {history.map((h) => (
        <li key={h.id} className="text-sm">
          <p className="font-medium">{h.label_hu}</p>
          <p className="text-metadata text-steel-500 font-mono">
            {formatDateTime(h.entered_at)}
            {h.left_at ? ` → ${formatDateTime(h.left_at)}` : ' → …'}
            {h.entered_by_name ? ` · ${h.entered_by_name}` : ''}
          </p>
          {h.note && <p className="mt-0.5 text-steel-900">{h.note}</p>}
        </li>
      ))}
    </ul>
  );
}
