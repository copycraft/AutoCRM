'use client';

// Ctrl/⌘+K palette: backend-ranked search hits plus instant destinations and
// new-record actions in one list. ↑↓ moves, Enter runs the highlighted row,
// Esc closes. Search queries join the same recent list as the header search.

import * as Dialog from '@radix-ui/react-dialog';
import { useLocale, useTranslations } from 'next-intl';
import { usePathname, useRouter } from 'next/navigation';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { FilePlus2, LayoutDashboard, Mail, MonitorPlay, BarChart3, Package, Target } from 'lucide-react';
import { searchApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { useRecent } from '@/hooks/useRecent';
import { useRestoreFocus } from '@/hooks/useRestoreFocus';
import { canEditLeads, canEditOrders, canEditPartners, useAuth } from '@/lib/auth/context';
import { cn } from '@/lib/utils/format';

export interface PaletteAction {
  id: string;
  href: string;
  label: string;
  hint: string;
  icon: typeof LayoutDashboard;
}

export function matchActions(q: string, actions: PaletteAction[]): PaletteAction[] {
  const needle = q.trim().toLowerCase();
  if (!needle) return actions;
  return actions.filter(
    (a) =>
      a.label.toLowerCase().includes(needle) ||
      a.hint.toLowerCase().includes(needle) ||
      a.id.includes(needle),
  );
}

export interface PaletteHit {
  href: string;
  title: string;
  sub: string;
}

export function CommandPalette() {
  const t = useTranslations('dashboard');
  const tq = useTranslations('qol');
  const tn = useTranslations('navigation');
  const to = useTranslations('orders');
  const tl = useTranslations('leads');
  const tp = useTranslations('partners');
  const locale = useLocale();
  const pathname = usePathname();
  const router = useRouter();
  const { user } = useAuth();
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState('');
  const [active, setActive] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  const debouncedQ = useDebouncedValue(q.trim(), 250);
  const ready = debouncedQ.length >= 2;
  const { push: pushRecent } = useRecent('autocrm:recent-search', 6);
  useRestoreFocus(open);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setOpen((o) => !o);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  // New page, new palette.
  useEffect(() => {
    setOpen(false);
    setQ('');
    setActive(0);
  }, [pathname]);

  const results = useQuery({
    queryKey: qk.search(debouncedQ),
    queryFn: ({ signal }) => searchApi.global(debouncedQ, { signal }),
    enabled: ready && open,
  });

  const actions = useMemo<PaletteAction[]>(() => {
    const list: PaletteAction[] = [
      { id: 'dashboard', href: `/${locale}`, label: tn('dashboard'), hint: 'g d', icon: LayoutDashboard },
      { id: 'orders', href: `/${locale}/orders`, label: tn('orders'), hint: 'g o', icon: Package },
      { id: 'leads', href: `/${locale}/leads`, label: tn('leads'), hint: 'g l', icon: Target },
      { id: 'emails', href: `/${locale}/emails`, label: tn('emails'), hint: 'g e', icon: Mail },
      { id: 'board', href: `/${locale}/board`, label: tn('board'), hint: 'g b', icon: MonitorPlay },
      { id: 'reports', href: `/${locale}/reports`, label: tn('reports'), hint: 'g r', icon: BarChart3 },
    ];
    if (canEditOrders(user)) {
      list.push({ id: 'new-order', href: `/${locale}/orders/new`, label: to('newOrder'), hint: 'n', icon: FilePlus2 });
    }
    if (canEditLeads(user)) {
      list.push({ id: 'new-lead', href: `/${locale}/leads/new`, label: tl('newLead'), hint: 'n', icon: FilePlus2 });
    }
    if (canEditPartners(user)) {
      list.push({ id: 'new-partner', href: `/${locale}/partners/new`, label: tp('newPartner'), hint: 'n', icon: FilePlus2 });
    }
    return list;
  }, [locale, tn, to, tl, tp, user]);

  const hits: PaletteHit[] = useMemo(() => {
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

  const visibleActions = useMemo(() => matchActions(debouncedQ, actions), [debouncedQ, actions]);
  const total = visibleActions.length + hits.length;

  useEffect(() => {
    setActive(0);
  }, [debouncedQ, results.data, open]);

  useEffect(() => {
    const el = listRef.current?.querySelector<HTMLElement>(`[data-index="${active}"]`);
    el?.scrollIntoView({ block: 'nearest' });
  }, [active]);

  const run = (index: number) => {
    if (index < visibleActions.length) {
      const a = visibleActions[index];
      if (!a) return;
      setOpen(false);
      router.push(a.href);
      return;
    }
    const hit = hits[index - visibleActions.length];
    if (!hit) return;
    if (debouncedQ) pushRecent(debouncedQ);
    setOpen(false);
    router.push(hit.href);
  };

  return (
    <Dialog.Root
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (!next) setQ('');
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-steel-900/40" />
        <Dialog.Content
          aria-label={tq('paletteTitle')}
          className="card fixed left-1/2 top-[15vh] z-50 max-h-[70vh] w-[90vw] max-w-lg -translate-x-1/2 overflow-hidden"
        >
          <div className="border-b border-steel-200 p-3">
            <Dialog.Title className="sr-only">{tq('paletteTitle')}</Dialog.Title>
            <input
              autoFocus
              value={q}
              onChange={(e) => setQ(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'ArrowDown' && total > 0) {
                  e.preventDefault();
                  setActive((a) => (a + 1) % total);
                } else if (e.key === 'ArrowUp' && total > 0) {
                  e.preventDefault();
                  setActive((a) => (a - 1 + total) % total);
                } else if (e.key === 'Home' && total > 0) {
                  e.preventDefault();
                  setActive(0);
                } else if (e.key === 'End' && total > 0) {
                  e.preventDefault();
                  setActive(total - 1);
                } else if (e.key === 'Enter' && total > 0) {
                  run(active);
                }
              }}
              placeholder={tq('palettePlaceholder')}
              aria-label={tq('palettePlaceholder')}
              role="combobox"
              aria-expanded
              aria-controls="cmd-palette-list"
              className="w-full bg-transparent text-body outline-none placeholder:text-steel-500"
            />
          </div>
          <div
            ref={listRef}
            id="cmd-palette-list"
            role="listbox"
            aria-label={tq('paletteTitle')}
            className="max-h-[55vh] overflow-y-auto p-1.5"
          >
            {visibleActions.length > 0 && (
              <div>
                <p className="px-2 py-1 text-metadata font-medium text-steel-500">
                  {tq('paletteActions')}
                </p>
                <ul>
                  {visibleActions.map((a, i) => {
                    const Icon = a.icon;
                    const isActive = i === active;
                    return (
                      <li key={a.id} data-index={i}>
                        <button
                          type="button"
                          role="option"
                          aria-selected={isActive}
                          onMouseEnter={() => setActive(i)}
                          onClick={() => run(i)}
                          className={cn(
                            'flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left',
                            isActive ? 'bg-steel-200/60' : 'hover:bg-steel-200/30',
                          )}
                        >
                          <Icon className="h-4 w-4 shrink-0 text-steel-500" aria-hidden />
                          <span className="flex-1 text-body font-medium">{a.label}</span>
                          <kbd className="rounded border border-steel-200 px-1.5 font-mono text-metadata text-steel-500">
                            {a.hint}
                          </kbd>
                        </button>
                      </li>
                    );
                  })}
                </ul>
              </div>
            )}
            {ready && (
              <div className="mt-1">
                <p className="px-2 py-1 text-metadata font-medium text-steel-500">
                  {tq('paletteResults')}
                </p>
                {results.isLoading ? (
                  <p className="px-2.5 py-2 text-metadata text-steel-500">{t('searchLoading')}</p>
                ) : hits.length === 0 ? (
                  <p className="px-2.5 py-2 text-body text-steel-500">{t('searchEmpty')}</p>
                ) : (
                  <ul>
                    {hits.map((h, i) => {
                      const index = visibleActions.length + i;
                      const isActive = index === active;
                      return (
                        <li key={h.href} data-index={index}>
                          <button
                            type="button"
                            role="option"
                            aria-selected={isActive}
                            onMouseEnter={() => setActive(index)}
                            onClick={() => run(index)}
                            className={cn(
                              'block w-full rounded-lg px-2.5 py-2 text-left',
                              isActive ? 'bg-steel-200/60' : 'hover:bg-steel-200/30',
                            )}
                          >
                            <span className="block text-body font-medium">{h.title}</span>
                            {h.sub && (
                              <span className="block truncate text-metadata text-steel-500">{h.sub}</span>
                            )}
                          </button>
                        </li>
                      );
                    })}
                  </ul>
                )}
              </div>
            )}
            {!ready && visibleActions.length === 0 && (
              <p className="px-2.5 py-2 text-body text-steel-500">{tq('paletteEmpty')}</p>
            )}
            <p className="border-t border-steel-200 px-2.5 pt-1.5 text-metadata text-steel-500">
              {tq('searchHintNav')}
            </p>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export function PaletteHint() {
  const tq = useTranslations('qol');
  return (
    <span className="inline-flex items-center gap-1 text-metadata text-steel-500">
      <kbd className="rounded border border-steel-200 px-1.5 font-mono text-metadata text-steel-500">
        Ctrl K
      </kbd>
      {tq('paletteHint')}
    </span>
  );
}
