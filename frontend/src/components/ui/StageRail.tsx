'use client';

// StageRail — signature traveller component (FRONTEND_PLAN.md §12).
// Renders BACKEND stage definitions + history. Never hardcodes stages.

import { Check, Circle } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { cn } from '@/lib/utils/format';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { EmailValue } from '@/components/ui/ContactLinks';
import type { Blocker, StageDefinition, StageEntry } from '@/lib/api/types';

export function StageRail({
  stages,
  currentKey,
  daysInStage,
  openBlockers,
  history,
}: {
  stages: StageDefinition[];
  currentKey: string;
  daysInStage: number;
  openBlockers: number;
  /** Stage history, oldest first — drives reached state, not position arithmetic. */
  history: StageEntry[];
}) {
  const t = useTranslations('stageRail');
  const visited = new Set(history.map((h) => h.stage_key));
  const ordered = [...stages]
    .filter((s) => s.is_active || s.key === currentKey)
    .sort((a, b) => a.position - b.position);

  return (
    <ol className="space-y-0" aria-label={t('title')}>
      {ordered.map((s, i) => {
        // Reached = present in history. An exit stage (cancelled/lost) terminates
        // the journey: anything never visited renders as never-reached, not done.
        const done = s.key !== currentKey && visited.has(s.key);
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
              <p className={cn('text-body', current ? 'font-semibold' : 'font-medium')}>{s.label_hu}</p>
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

/**
 * Horizontal traveller for narrow screens: stage chips in journey order plus
 * the open blockers inline (what + who + due), not just a count.
 */
export function TravellerStrip({
  stages,
  currentKey,
  history,
  blockers,
}: {
  stages: StageDefinition[];
  currentKey: string;
  history: StageEntry[];
  blockers: Blocker[];
}) {
  const t = useTranslations('stageRail');
  const visited = new Set(history.map((h) => h.stage_key));
  const ordered = [...stages]
    .filter((s) => s.is_active || s.key === currentKey)
    .sort((a, b) => a.position - b.position);
  return (
    <section aria-label={t('title')} className="space-y-2">
      <ol className="flex gap-2 overflow-x-auto pb-1">
        {ordered.map((s) => {
          const done = s.key !== currentKey && visited.has(s.key);
          const current = s.key === currentKey;
          return (
            <li
              key={s.key}
              aria-current={current ? 'step' : undefined}
              className={cn(
                'shrink-0 rounded-lg border px-2.5 py-1 text-body',
                done && 'border-done/40 bg-done/10 text-done',
                current && 'border-steel-900 bg-steel-900 font-semibold text-surface',
                !done && !current && 'border-steel-200 bg-surface text-steel-500',
              )}
            >
              {s.label_hu}
            </li>
          );
        })}
      </ol>
      {blockers.length > 0 && (
        <ul className="space-y-1">
          {blockers.map((b) => (
            <li key={b.id} className="text-metadata text-steel-900">
              <span className="font-medium">{b.what}</span>{' '}
              <span className="text-steel-500">
                · {b.responsible_partner_name}
                {b.responsible_partner_name && b.responsible_email ? ' · ' : ''}
                {b.responsible_email && <EmailValue value={b.responsible_email} />}
                {!b.responsible_partner_name && !b.responsible_email && '—'}
                {b.due_date ? (
                  <>
                    {' '}· <DateDisplay value={b.due_date} />
                  </>
                ) : (
                  ''
                )}
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

export function StageHistoryList({ history }: { history: StageEntry[] }) {
  const t = useTranslations('stageRail');
  if (history.length === 0) return <p className="text-metadata text-steel-500">{t('noHistory')}</p>;
  return (
    <ul className="space-y-3">
      {history.map((h) => (
        <li key={h.id} className="text-body">
          <p className="font-medium">{h.label_hu}</p>
          <p className="text-metadata text-steel-500">
            <DateDisplay withTime value={h.entered_at} />
            {h.left_at ? (
              <>
                {' → '}
                <DateDisplay withTime value={h.left_at} />
              </>
            ) : (
              ' → …'
            )}
            {h.entered_by_name ? ` · ${h.entered_by_name}` : ''}
          </p>
          {h.note && <p className="mt-0.5 text-steel-900">{h.note}</p>}
        </li>
      ))}
    </ul>
  );
}
