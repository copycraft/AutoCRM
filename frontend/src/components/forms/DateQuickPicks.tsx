'use client';

import { useTranslations } from 'next-intl';

/** Budapest-calendar ISO date (YYYY-MM-DD) shifted by `days`. */
export function budapestIsoPlus(days: number): string {
  const today = new Intl.DateTimeFormat('en-CA', { timeZone: 'Europe/Budapest' }).format(new Date());
  const [y, m, d] = today.split('-').map(Number);
  const ms = Date.UTC(y ?? 1970, (m ?? 1) - 1, d ?? 1) + days * 86_400_000;
  return new Date(ms).toISOString().slice(0, 10);
}

/** Ma / +7 / +30 shortcuts for date inputs. Caller writes the picked ISO date. */
export function DateQuickPicks({ onPick }: { onPick: (iso: string) => void }) {
  const t = useTranslations('qol');
  const picks = [
    { days: 0, label: t('dateToday') },
    { days: 7, label: t('datePlus7') },
    { days: 30, label: t('datePlus30') },
  ];
  return (
    <span className="inline-flex items-center gap-1" role="group" aria-label={t('dateQuick')}>
      {picks.map((p) => (
        <button
          key={p.days}
          type="button"
          className="btn-ghost btn-sm !px-2"
          onClick={() => onPick(budapestIsoPlus(p.days))}
        >
          {p.label}
        </button>
      ))}
    </span>
  );
}
