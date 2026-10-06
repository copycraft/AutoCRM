'use client';

// A status list down the side of a page, the way MiniCRM showed them: headed groups of
// coloured entries with counts. The entries are worked out by the server, not set by
// hand; clicking one filters the page.

import { cn } from '@/lib/utils/format';

export interface Bucket {
  key: string;
  label: string;
  color: string;
}

export function BucketList({
  title,
  groups,
  counts,
  value,
  onChange,
  allLabel,
}: {
  title: string;
  groups: { title: string; buckets: Bucket[] }[];
  counts: Record<string, number>;
  /** The selected bucket key; '' is everything. */
  value: string;
  onChange: (key: string) => void;
  allLabel: string;
}) {
  const total = Object.values(counts).reduce((a, b) => a + b, 0);
  return (
    <nav className="space-y-4 text-body" aria-label={title}>
      <button
        type="button"
        className={cn('w-full rounded px-1 py-0.5 text-left hover:bg-steel-200/40', value === '' && 'bg-steel-200/60 font-semibold')}
        onClick={() => onChange('')}
        aria-pressed={value === ''}
      >
        {allLabel} <span className="font-mono text-metadata text-steel-500">({total})</span>
      </button>
      {groups.map((g) => (
        <section key={g.title} className="border-t border-steel-200 pt-3">
          <h3 className="mb-1.5 font-semibold text-steel-900">{g.title}</h3>
          <ul className="space-y-0.5">
            {g.buckets.map((b) => (
              <li key={b.key} className="flex items-center gap-2">
                <span className="h-4 w-4 shrink-0 rounded ring-1 ring-inset ring-black/10" style={{ backgroundColor: b.color }} />
                <button
                  type="button"
                  className={cn(
                    'min-w-0 flex-1 truncate rounded px-1 py-0.5 text-left hover:bg-steel-200/40',
                    value === b.key && 'bg-steel-200/60 font-semibold',
                  )}
                  aria-pressed={value === b.key}
                  onClick={() => onChange(value === b.key ? '' : b.key)}
                >
                  {b.label} <span className="font-mono text-metadata text-steel-500">({counts[b.key] ?? 0})</span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      ))}
    </nav>
  );
}

/** A small coloured label for a row's bucket. */
export function BucketBadge({ bucket }: { bucket: Bucket | undefined }) {
  if (!bucket) return null;
  return (
    <span className="inline-flex items-center gap-1.5 rounded-md bg-steel-200/50 px-1.5 py-0.5 text-metadata font-medium text-steel-900">
      <span className="h-2.5 w-2.5 rounded-sm ring-1 ring-inset ring-black/10" style={{ backgroundColor: bucket.color }} />
      {bucket.label}
    </span>
  );
}
