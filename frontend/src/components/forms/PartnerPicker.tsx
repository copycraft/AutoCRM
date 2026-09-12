'use client';

import { useId, useRef, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useTranslations } from 'next-intl';
import { errorMessage } from '@/lib/api/errors';
import { partnersApi } from '@/lib/api/endpoints';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { cn } from '@/lib/utils/format';

export interface PartnerOption {
  id: number;
  name: string;
}

// Search-as-you-type partner picker backed by GET /partners (q matches
// name/tax/e-mail/city server-side). No local fake list.
// WAI-ARIA combobox: labelled input, listbox results, full keyboard support
// (arrows move, Enter selects, Escape closes).
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
  const ter = useTranslations('errors');
  const baseId = useId();
  const inputId = `${baseId}-input`;
  const listId = `${baseId}-list`;
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState('');
  const [active, setActive] = useState(-1);
  const listRef = useRef<HTMLDivElement>(null);
  const debounced = useDebouncedValue(q);
  const search = useQuery({
    queryKey: ['partner-picker', debounced],
    queryFn: () => partnersApi.list({ q: debounced || undefined, limit: 8 }),
    enabled: open,
  });
  const results = search.data?.items ?? [];

  function pick(index: number) {
    const p = results[index];
    if (!p) return;
    onChange({ id: p.id, name: p.name });
    setOpen(false);
    setQ('');
    setActive(-1);
  }

  function onKeyDown(e: React.KeyboardEvent) {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      if (!open) {
        setOpen(true);
        return;
      }
      if (results.length === 0) return;
      const next =
        e.key === 'ArrowDown'
          ? (active + 1) % results.length
          : (active - 1 + results.length) % results.length;
      setActive(next);
      listRef.current
        ?.querySelector(`[data-index="${next}"]`)
        ?.scrollIntoView({ block: 'nearest' });
    } else if (e.key === 'Enter') {
      if (open && active >= 0) {
        e.preventDefault();
        pick(active);
      }
    } else if (e.key === 'Escape') {
      setOpen(false);
      setActive(-1);
    }
  }

  return (
    <div className="relative">
      <label className="label" htmlFor={inputId}>{label}</label>
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
            id={inputId}
            className="input"
            role="combobox"
            aria-expanded={open}
            aria-controls={listId}
            aria-activedescendant={active >= 0 ? `${baseId}-opt-${active}` : undefined}
            autoComplete="off"
            placeholder={t('searchPlaceholder')}
            value={q}
            onFocus={() => setOpen(true)}
            onChange={(e) => {
              setQ(e.target.value);
              setOpen(true);
              setActive(-1);
            }}
            onKeyDown={onKeyDown}
            onBlur={() => setOpen(false)}
          />
          {search.isError && (
            <p className="mt-1 text-xs text-steel-900" role="alert">
              {errorMessage(search.error, ter, ter('unknownError'))}
            </p>
          )}
          {open && (
            <div
              ref={listRef}
              id={listId}
              role="listbox"
              aria-label={label}
              className="absolute z-10 mt-1 max-h-56 w-full overflow-auto rounded-lg border border-steel-200 bg-surface shadow-card"
            >
              {search.isLoading && <p className="px-3 py-2 text-sm text-steel-500">{tc('loading')}</p>}
              {results.map((p, i) => (
                <div
                  key={p.id}
                  id={`${baseId}-opt-${i}`}
                  data-index={i}
                  role="option"
                  aria-selected={i === active}
                  tabIndex={-1}
                  className={cn(
                    'block w-full cursor-pointer px-3 py-2 text-left text-sm hover:bg-panel',
                    i === active && 'bg-panel',
                  )}
                  onMouseDown={(e) => {
                    // Select before the input blur closes the listbox.
                    e.preventDefault();
                    pick(i);
                  }}
                  onMouseEnter={() => setActive(i)}
                >
                  <span className="font-medium">{p.name}</span>
                  <span className="text-steel-500"> · #{p.id}</span>
                </div>
              ))}
              {search.data && results.length === 0 && (
                <p className="px-3 py-2 text-sm text-steel-500">{te('searchNoResults')}</p>
              )}
              <button
                type="button"
                tabIndex={-1}
                className="block w-full px-3 py-2 text-left text-xs text-steel-500 hover:bg-panel"
                onMouseDown={(e) => {
                  e.preventDefault();
                  setOpen(false);
                }}
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
