'use client';

import { useEffect, useMemo, useState } from 'react';
import Link from 'next/link';
import { useLocale, useTranslations } from 'next-intl';
import { useSearchParams } from 'next/navigation';
import { ArrowLeft, BookOpen, Search } from 'lucide-react';
import { cn } from '@/lib/utils/format';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { KNOWLEDGE_ARTICLES, KNOWLEDGE_GROUPS } from './articles';

function fold(s: string): string {
  return s
    .toLowerCase()
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .replace(/[^a-z0-9\s]/g, ' ');
}

export function KnowledgeBase() {
  const t = useTranslations('knowledge');
  const locale = useLocale();
  const searchParams = useSearchParams();
  const initialSlug = searchParams.get('cikk');
  const [slug, setSlug] = useState<string | null>(initialSlug);
  const [query, setQuery] = useState('');
  const debounced = useDebouncedValue(query, 250);

  useEffect(() => {
    setSlug(searchParams.get('cikk'));
  }, [searchParams]);

  const filtered = useMemo(() => {
    const q = fold(debounced.trim());
    if (!q) return KNOWLEDGE_ARTICLES;
    const words = q.split(/\s+/).filter(Boolean);
    return KNOWLEDGE_ARTICLES.filter((a) => {
      const hay = fold(`${a.title} ${a.summary} ${a.keywords.join(' ')} ${a.body.flatMap((b) => b.paragraphs).join(' ')}`);
      return words.every((w) => hay.includes(w));
    });
  }, [debounced]);

  const active = slug ? KNOWLEDGE_ARTICLES.find((a) => a.slug === slug) ?? null : null;

  const open = (next: string) => {
    setSlug(next);
    try {
      const url = new URL(window.location.href);
      url.searchParams.set('cikk', next);
      window.history.replaceState(null, '', url.toString());
    } catch {
      /* best-effort */
    }
  };

  const back = () => {
    setSlug(null);
    try {
      const url = new URL(window.location.href);
      url.searchParams.delete('cikk');
      window.history.replaceState(null, '', url.toString());
    } catch {
      /* best-effort */
    }
  };

  if (active) {
    return (
      <div className="grid gap-6 lg:grid-cols-[240px_minmax(0,1fr)]">
        <nav aria-label={t('contents')} className="space-y-1 lg:sticky lg:top-4 lg:self-start">
          <button onClick={back} className="btn-ghost btn-sm mb-3">
            <ArrowLeft className="h-4 w-4" aria-hidden />
            {t('backToList')}
          </button>
          {KNOWLEDGE_ARTICLES.map((a) => (
            <button
              key={a.slug}
              onClick={() => open(a.slug)}
              aria-current={a.slug === active.slug ? 'page' : undefined}
              className={cn(
                'block w-full rounded-lg px-3 py-2 text-left text-body transition-colors',
                a.slug === active.slug ? 'bg-steel-900 text-surface' : 'text-steel-900 hover:bg-panel',
              )}
            >
              {a.title}
            </button>
          ))}
        </nav>
        <article className="card">
          <div className="card-header">
            <p className="text-metadata text-steel-500">{active.group}</p>
            <h2 className="text-record-title font-semibold tracking-tight">{active.title}</h2>
            <p className="mt-1 text-body text-steel-600">{active.summary}</p>
          </div>
          <div className="card-content space-y-5">
            {active.body.map((section, i) => (
              <section key={i} className="space-y-2">
                {section.heading && <h3 className="text-section font-semibold">{section.heading}</h3>}
                {section.paragraphs.map((p, j) => (
                  <p key={j} className="text-body leading-relaxed text-steel-900">
                    {p}
                  </p>
                ))}
              </section>
            ))}
            {active.links.length > 0 && (
              <section className="space-y-2 border-t border-steel-200 pt-4">
                <h3 className="text-section font-semibold">{t('related')}</h3>
                <div className="flex flex-wrap gap-2">
                  {active.links.map((l) => (
                    <Link key={l.href + l.label} href={`/${locale}${l.href.replace(/^\/hu/, '')}`} className="btn-ghost btn-sm">
                      {l.label}
                    </Link>
                  ))}
                </div>
              </section>
            )}
          </div>
        </article>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="relative max-w-xl">
        <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-steel-400" aria-hidden />
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t('searchPlaceholder')}
          aria-label={t('searchPlaceholder')}
          className="input pl-9"
        />
      </div>
      {filtered.length === 0 ? (
        <p className="text-body text-steel-500">{t('noResults')}</p>
      ) : (
        KNOWLEDGE_GROUPS.map((group) => {
          const items = filtered.filter((a) => a.group === group);
          if (items.length === 0) return null;
          return (
            <section key={group} aria-label={group} className="space-y-3">
              <h2 className="text-section font-semibold uppercase tracking-wider text-steel-500">{group}</h2>
              <div className="grid gap-3 md:grid-cols-2">
                {items.map((a) => (
                  <button
                    key={a.slug}
                    onClick={() => open(a.slug)}
                    className="card p-4 text-left transition-colors hover:border-steel-400"
                  >
                    <span className="flex items-start gap-3">
                      <BookOpen className="mt-0.5 h-4 w-4 shrink-0 text-steel-500" aria-hidden />
                      <span>
                        <span className="block text-body font-semibold text-steel-900">{a.title}</span>
                        <span className="mt-1 block text-metadata text-steel-500">{a.summary}</span>
                      </span>
                    </span>
                  </button>
                ))}
              </div>
            </section>
          );
        })
      )}
    </div>
  );
}
