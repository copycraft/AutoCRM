'use client';

import { useTranslations } from 'next-intl';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { daysSince } from '@/lib/utils/format';

/** Budapest calendar day for grouping (`en-CA` yields YYYY-MM-DD). */
export function dayKey(iso: string): string {
  try {
    return new Intl.DateTimeFormat('en-CA', { timeZone: 'Europe/Budapest' }).format(new Date(iso));
  } catch {
    return iso.slice(0, 10);
  }
}

/** Stable day groups in first-seen order — callers pass newest-first lists. */
export function groupByDay<T>(items: T[], getAt: (item: T) => string): { day: string; items: T[] }[] {
  const groups: { day: string; items: T[] }[] = [];
  for (const item of items) {
    const day = dayKey(getAt(item));
    const last = groups[groups.length - 1];
    if (last && last.day === day) last.items.push(item);
    else groups.push({ day, items: [item] });
  }
  return groups;
}

/** "Ma · 2026.09.13.", "Tegnap · …", else the bare date. */
export function DayLabel({ day }: { day: string }) {
  const t = useTranslations('qol');
  const age = daysSince(`${day}T12:00:00`);
  const rel = age === 0 ? t('today') : age === 1 ? t('yesterday') : null;
  return (
    <span className="text-metadata font-medium text-steel-500">
      {rel ? `${rel} · ` : ''}
      <DateDisplay value={day} />
    </span>
  );
}
