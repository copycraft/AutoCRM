'use client';

import type { ReactNode } from 'react';
import type { StageDefinition } from '@/lib/api/types';
import type { Bucket } from '@/components/tags/BucketList';

/**
 * A record list with its status list down the right side, the way the invoice pages
 * show their buckets: the filters and the table on the left, where things stand on the
 * right. On a phone the status list comes first, above the records.
 */
export function WithStatusAside({ aside, children }: { aside: ReactNode; children: ReactNode }) {
  return (
    <div className="grid grid-cols-1 gap-6 lg:grid-cols-[minmax(0,1fr)_15rem]">
      <div className="min-w-0 space-y-6">{children}</div>
      <aside className="card order-first self-start p-3 lg:sticky lg:top-6 lg:order-none">{aside}</aside>
    </div>
  );
}

/** Open stages in deepening blues, by position; the brand blue sits in the middle. */
const OPEN_COLORS = ['#93c5fd', '#60a5fa', '#3d6eee', '#2563eb', '#1d4ed8', '#1e3a8a'];
const WON_COLOR = '#16a34a';
const EXIT_COLOR = '#94a3b8';

/**
 * Stage definitions as status-list groups, MiniCRM's three: open, successful
 * (terminal), unsuccessful (exit). From the stage flags, never stage names; inactive
 * stages and empty groups are left out.
 */
export function stageGroups(
  defs: StageDefinition[] | undefined,
  titles: { open: string; won: string; lost: string },
): { title: string; buckets: Bucket[] }[] {
  const active = [...(defs ?? [])].filter((d) => d.is_active).sort((a, b) => a.position - b.position);
  const open = active.filter((d) => !d.is_terminal);
  const groups = [
    {
      title: titles.open,
      buckets: open.map((d, i) => ({
        key: d.key,
        label: d.label_hu,
        color: OPEN_COLORS[Math.min(i, OPEN_COLORS.length - 1)] ?? OPEN_COLORS[0]!,
      })),
    },
    {
      title: titles.won,
      buckets: active
        .filter((d) => d.is_terminal && !d.is_exit)
        .map((d) => ({ key: d.key, label: d.label_hu, color: WON_COLOR })),
    },
    {
      title: titles.lost,
      buckets: active
        .filter((d) => d.is_exit)
        .map((d) => ({ key: d.key, label: d.label_hu, color: EXIT_COLOR })),
    },
  ];
  return groups.filter((g) => g.buckets.length > 0);
}

/** `[{key, count}]` from a counts endpoint as the map `BucketList` reads. */
export function countMap(items: { key: string; count: number }[] | undefined): Record<string, number> {
  return Object.fromEntries((items ?? []).map((c) => [c.key, c.count]));
}
