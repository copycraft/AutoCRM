// StageRail — signature traveller component (§26, §92).
// Renders BACKEND stage definitions + history. Never hardcodes stages.

import { Check, Circle, CircleDot } from 'lucide-react';
import { cn, formatDateTime } from '@/lib/utils/format';
import type { StageDefinition, StageEntry } from '@/types/api';

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
  const ordered = [...stages].sort((a, b) => a.position - b.position);
  const currentIdx = ordered.findIndex((s) => s.key === currentKey);

  return (
    <ol className="space-y-0" aria-label="Fázisok">
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
                <Check className="h-[19px] w-[19px] text-done" aria-label="Kész" />
              ) : current ? (
                <CircleDot className="h-[19px] w-[19px] text-signal" aria-label="Aktuális" />
              ) : (
                <Circle className="h-[19px] w-[19px] text-steel-200" aria-hidden />
              )}
            </span>
            <div className="min-w-0">
              <p className={cn('text-sm', current ? 'font-semibold' : 'font-medium')}>{s.label_hu}</p>
              {current && (
                <p className="text-metadata text-steel-500">
                  {daysInStage} napja itt
                  {openBlockers > 0 && ` · ${openBlockers} nyitott akadály`}
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
  if (history.length === 0) return <p className="text-sm text-steel-500">Nincs fáziselőzmény.</p>;
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
