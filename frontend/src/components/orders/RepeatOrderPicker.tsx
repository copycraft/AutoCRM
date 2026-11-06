'use client';

// On the new-order page: start from an earlier project instead of a blank form. Search by
// number, customer, title or plate, pick one, and the copy dialog takes it from there.

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useQuery } from '@tanstack/react-query';
import { ordersApi } from '@/lib/api/endpoints';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { CopyOrderDialog } from './CopyOrderDialog';

export function RepeatOrderPicker() {
  const t = useTranslations('repeat');
  const [q, setQ] = useState('');
  const [picked, setPicked] = useState<{ id: number; number: string; title: string } | null>(null);
  const term = useDebouncedValue(q.trim(), 300);
  const results = useQuery({
    queryKey: ['orders', 'repeat-search', term],
    queryFn: () => ordersApi.list({ q: term, limit: 8 }),
    enabled: term.length >= 2,
  });

  return (
    <section className="card mb-6">
      <div className="card-header">
        <h2 className="text-section font-semibold">{t('fromEarlier')}</h2>
        <p className="text-metadata text-steel-500">{t('fromEarlierHint')}</p>
      </div>
      <div className="card-content space-y-2">
        <input
          className="input"
          type="search"
          placeholder={t('searchPlaceholder')}
          aria-label={t('fromEarlier')}
          value={q}
          onChange={(e) => setQ(e.target.value)}
        />
        {term.length >= 2 && (
          <ul className="divide-y divide-steel-200">
            {(results.data?.items ?? []).map((o) => (
              <li key={o.id} className="flex items-center justify-between gap-3 py-2">
                <span className="min-w-0 truncate text-body">
                  <span className="font-mono">#{o.number}</span> · {o.title}
                  <span className="text-steel-500"> · {o.partner_name}</span>
                </span>
                <button
                  type="button"
                  className="btn-secondary btn-sm shrink-0"
                  onClick={() => setPicked({ id: o.id, number: o.number, title: o.title })}
                >
                  {t('useThis')}
                </button>
              </li>
            ))}
            {results.data && results.data.items.length === 0 && (
              <li className="py-2 text-metadata text-steel-500">{t('noResults')}</li>
            )}
          </ul>
        )}
      </div>
      {picked && <CopyOrderDialog source={picked} onClose={() => setPicked(null)} />}
    </section>
  );
}
