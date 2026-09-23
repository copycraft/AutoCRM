'use client';

// The header search: one input, orders + partners + leads, backend-ranked.
// `/` focuses from anywhere (unless typing), ↑↓ moves, Enter opens the
// highlighted hit, Esc closes. Recent queries persist locally (no PII beyond
// what the user typed) and show when the box is empty.

import Link from 'next/link';
import { usePathname, useRouter } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { searchApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { useRecent } from '@/hooks/useRecent';
import { cn } from '@/lib/utils/format';

export function GlobalSearch() {
  const t = useTranslations('dashboard');
  const tq = useTranslations('qol');
  const locale = useLocale();
  const pathname = usePathname();
  const router = useRouter();
  const [q, setQ] = useState('');
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const debouncedQ = useDebouncedValue(q.trim(), 250);
  const ready = debouncedQ.length >= 2;
  const { items: recent, push: pushRecent, clear: clearRecent } = useRecent(
    'autocrm:recent-search',
    6,
  );

  const results = useQuery({
    queryKey: qk.search(debouncedQ),
    queryFn: ({ signal }) => searchApi.global(debouncedQ, { signal }),
    enabled: ready && open,
  });

  // New page, new search.
  useEffect(() => {
    setQ('');
    setOpen(false);
    setActive(0);
  }, [pathname]);

  useEffect(() => {
    setActive(0);
  }, [debouncedQ, results.data]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const typing =
        target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable);
      if (e.key === '/' && !typing) {
        e.preventDefault();
        inputRef.current?.focus();
      } else if (e.key === 'Escape') {
        setOpen(false);
        inputRef.current?.blur();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const flat: { href: string; title: string; sub: string }[] = useMemo(() => {
    const items = results.data;
    if (!items) return [];
    return [
      ...items.orders.map((o) => ({
        href: `/${locale}/orders/${o.id}`,
        title: `#${o.number} · ${o.plate ?? '—'}`,
        sub: `${o.title} · ${o.stage_label}`,
      })),
      ...items.partners.map((p) => ({
        href: `/${locale}/partners/${p.id}`,
        title: p.name,
        sub: [p.kind === 'business' ? 'Üzleti' : 'Magán', p.city].filter(Boolean).join(' · '),
      })),
      ...items.leads.map((l) => ({
        href: `/${locale}/leads/${l.id}`,
        title: l.title,
        sub: [l.contact_name, l.stage_label].filter(Boolean).join(' · '),
      })),
    ];
  }, [results.data, locale]);

  const groups: { label: string; from: number; to: number }[] = useMemo(() => {
    const o = results.data?.orders.length ?? 0;
    const p = results.data?.partners.length ?? 0;
    const l = results.data?.leads.length ?? 0;
    return [
      { label: t('searchOrders'), from: 0, to: o },
      { label: t('searchPartners'), from: o, to: o + p },
      { label: t('searchLeads'), from: o + p, to: o + p + l },
    ].filter((g) => g.to > g.from);
  }, [results.data, t]);

  const openHit = (index: number) => {
    const hit = flat[index];
    if (!hit) return;
    if (debouncedQ) pushRecent(debouncedQ);
    setOpen(false);
    router.push(hit.href);
  };

  const showRecent = q.trim() === '' && recent.length > 0;

  useEffect(() => {
    // Keep the highlighted row visible while arrowing through long lists.
    const el = listRef.current?.querySelector<HTMLElement>(`[data-index="${active}"]`);
    el?.scrollIntoView({ block: 'nearest' });
  }, [active]);

  return (
    <div className="relative px-3 pt-3">
      <div className="flex items-center gap-2 rounded-lg border border-steel-200 bg-panel px-3 py-2 focus-within:border-steel-900">
        <input
          ref={inputRef}
          value={q}
          onChange={(e) => {
            setQ(e.target.value);
            setOpen(true);
          }}
          onFocus={() => setOpen(true)}
          onKeyDown={(e) => {
            if (e.key === 'ArrowDown' && flat.length > 0) {
              e.preventDefault();
              setActive((a) => (a + 1) % flat.length);
            } else if (e.key === 'ArrowUp' && flat.length > 0) {
              e.preventDefault();
              setActive((a) => (a - 1 + flat.length) % flat.length);
            } else if (e.key === 'Home' && flat.length > 0) {
              e.preventDefault();
              setActive(0);
            } else if (e.key === 'End' && flat.length > 0) {
              e.preventDefault();
              setActive(flat.length - 1);
            } else if (e.key === 'Enter') {
              if (showRecent) {
                const pick = recent[active];
                if (pick) {
                  setQ(pick);
                  setOpen(true);
                }
              } else if (flat.length > 0) {
                openHit(active);
              }
            }
          }}
          placeholder={t('searchPlaceholder')}
          aria-label={t('searchPlaceholder')}
          role="combobox"
          aria-expanded={open}
          aria-controls="global-search-list"
          aria-activedescendant={flat.length > 0 ? `gs-hit-${active}` : undefined}
          className="w-full bg-transparent text-body outline-none placeholder:text-steel-500"
        />
        <kbd className="rounded border border-steel-200 px-1.5 font-mono text-metadata text-steel-500">
          {t('searchSlashHint')}
        </kbd>
      </div>

      {open && (q.trim() !== '' || showRecent) && (
        <>
          <button
            aria-label={t('searchClose')}
            className="fixed inset-0 z-40 cursor-default bg-transparent"
            onClick={() => setOpen(false)}
            tabIndex={-1}
          />
          <div
            ref={listRef}
            id="global-search-list"
            role="listbox"
            aria-label={t('searchPlaceholder')}
            className="absolute inset-x-3 top-full z-50 mt-1 max-h-[60vh] overflow-y-auto rounded-lg border border-steel-200 bg-surface shadow-lg"
          >
            {showRecent ? (
              <div>
                <p className="flex items-center justify-between border-b border-steel-200 bg-panel px-3 py-1 text-metadata font-medium text-steel-500">
                  <span>{tq('searchRecent')}</span>
                  <button
                    type="button"
                    className="underline hover:text-steel-900"
                    onClick={clearRecent}
                  >
                    {tq('searchClearRecent')}
                  </button>
                </p>
                <ul>
                  {recent.map((r, i) => (
                    <li key={r} data-index={i} id={`gs-hit-${i}`}>
                      <button
                        type="button"
                        role="option"
                        aria-selected={i === active}
                        className={cn(
                          'block w-full px-3 py-2 text-left',
                          i === active ? 'bg-steel-200/60' : 'hover:bg-steel-200/30',
                        )}
                        onMouseEnter={() => setActive(i)}
                        onClick={() => {
                          setQ(r);
                          setOpen(true);
                          inputRef.current?.focus();
                        }}
                      >
                        <span className="block text-body">{r}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            ) : !ready ? (
              <p className="px-3 py-2 text-metadata text-steel-500">{t('searchMinChars')}</p>
            ) : results.isLoading ? (
              <p className="px-3 py-2 text-metadata text-steel-500">{t('searchLoading')}</p>
            ) : flat.length === 0 ? (
              <p className="px-3 py-2 text-body text-steel-500">{t('searchEmpty')}</p>
            ) : (
              <>
                {groups.map((g) => (
                  <div key={g.label}>
                    <p className="border-b border-steel-200 bg-panel px-3 py-1 text-metadata font-medium text-steel-500">
                      {g.label}
                    </p>
                    <ul>
                      {flat.slice(g.from, g.to).map((r, i) => {
                        const index = g.from + i;
                        const isActive = index === active;
                        return (
                          <li key={r.href} data-index={index} id={`gs-hit-${index}`}>
                            <Link
                              href={r.href}
                              role="option"
                              aria-selected={isActive}
                              onMouseEnter={() => setActive(index)}
                              onClick={() => {
                                if (debouncedQ) pushRecent(debouncedQ);
                                setOpen(false);
                              }}
                              className={cn(
                                'block px-3 py-2',
                                isActive ? 'bg-steel-200/60' : 'hover:bg-steel-200/30',
                              )}
                            >
                              <span className="block text-body font-medium">{r.title}</span>
                              {r.sub && (
                                <span className="block truncate text-metadata text-steel-500">{r.sub}</span>
                              )}
                            </Link>
                          </li>
                        );
                      })}
                    </ul>
                  </div>
                ))}
                <p className="border-t border-steel-200 px-3 py-1.5 text-metadata text-steel-500">
                  {tq('searchHintNav')}
                </p>
              </>
            )}
          </div>
        </>
      )}
    </div>
  );
}
