'use client';

import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useTranslations } from 'next-intl';
import { partnersApi } from '@/lib/api/endpoints';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';

export interface PartnerOption {
  id: number;
  name: string;
}

// Search-as-you-type partner picker backed by GET /partners (q matches
// name/tax/e-mail/city server-side). No local fake list.
export function PartnerPicker({
  value,
  onChange,
  label,
}: {
  value: PartnerOption | null;
  onChange: (p: PartnerOption | null) => void;
  label: string;
}) {
  const t = useTranslations('partners');
  const tc = useTranslations('common');
  const te = useTranslations('emptyStates');
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState('');
  const debounced = useDebouncedValue(q);
  const search = useQuery({
    queryKey: ['partner-picker', debounced],
    queryFn: () => partnersApi.list({ q: debounced || undefined, limit: 8 }),
    enabled: open,
  });

  return (
    <div className="relative">
      <span className="label">{label}</span>
      {value ? (
        <div className="flex items-center justify-between gap-2 rounded-lg border border-steel-200 bg-surface px-3 py-2">
          <span className="text-sm font-medium truncate">{value.name}</span>
          <button type="button" className="btn-ghost btn-sm shrink-0" onClick={() => onChange(null)}>
            {tc('clear')}
          </button>
        </div>
      ) : (
        <>
          <input
            className="input"
            placeholder={t('searchPlaceholder')}
            value={q}
            onFocus={() => setOpen(true)}
            onChange={(e) => {
              setQ(e.target.value);
              setOpen(true);
            }}
          />
          {open && (
            <div className="absolute z-10 mt-1 max-h-56 w-full overflow-auto rounded-lg border border-steel-200 bg-surface shadow-card">
              {search.isLoading && <p className="px-3 py-2 text-sm text-steel-500">{tc('loading')}</p>}
              {search.data?.items.map((p) => (
                <button
                  key={p.id}
                  type="button"
                  className="block w-full px-3 py-2 text-left text-sm hover:bg-panel"
                  onClick={() => {
                    onChange({ id: p.id, name: p.name });
                    setOpen(false);
                    setQ('');
                  }}
                >
                  <span className="font-medium">{p.name}</span>
                  <span className="text-steel-500"> · #{p.id}</span>
                </button>
              ))}
              {search.data && search.data.items.length === 0 && (
                <p className="px-3 py-2 text-sm text-steel-500">{te('searchNoResults')}</p>
              )}
              <button
                type="button"
                className="block w-full px-3 py-2 text-left text-xs text-steel-500 hover:bg-panel"
                onClick={() => setOpen(false)}
              >
                {t('closePicker')}
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
}
