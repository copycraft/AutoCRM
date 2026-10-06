'use client';

// The lead tag lists, one column per market, edited in place: type a name and press Enter
// to add, click a name to rename, click the swatch to recolour, and give a tag the
// websites whose leads should get it automatically.

import { useState } from 'react';
import Link from 'next/link';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { ArrowDown, ArrowUp, Archive, Globe, Plus, RotateCcw, X } from 'lucide-react';
import { leadTagsApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { ErrorState } from '@/components/ui/ErrorState';
import { LoadingState } from '@/components/ui/LoadingState';
import { canEditLeads, useAuth } from '@/lib/auth/context';
import { sortMarkets, useMarketName } from './LeadTags';
import { IconButton, PALETTE, SwatchButton } from '@/components/tags/TagKit';
import type { LeadTag } from '@/lib/api/types';

export function LeadTagManager() {
  const t = useTranslations('leadTags');
  const ter = useTranslations('errors');
  const locale = useLocale();
  const marketName = useMarketName();
  const { user } = useAuth();
  const editable = canEditLeads(user);
  const qc = useQueryClient();
  const live = useQuery({ queryKey: qk.leadTags(), queryFn: () => leadTagsApi.list() });
  const archived = useQuery({ queryKey: qk.leadTags(true), queryFn: () => leadTagsApi.list(true) });
  const [extraMarkets, setExtraMarkets] = useState<string[]>([]);
  const [newMarket, setNewMarket] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ['lead-tags'] });
    void qc.invalidateQueries({ queryKey: ['leads'] });
    void qc.invalidateQueries({ queryKey: ['lead'] });
  };
  const onError = (e: unknown) => setError(errorMessage(e, ter, ter('unknownError')));
  const update = useMutation({
    mutationFn: ({ id, body }: { id: number; body: Parameters<typeof leadTagsApi.update>[1] }) =>
      leadTagsApi.update(id, body),
    onSuccess: () => {
      setError(null);
      refresh();
    },
    onError,
  });

  if (live.isPending) return <LoadingState />;
  if (live.isError) return <ErrorState error={live.error} onRetry={() => void live.refetch()} />;

  const tags = live.data.items;
  const markets = sortMarkets([...tags.map((x) => x.market), ...extraMarkets]);
  const archivedTags = archived.data?.items ?? [];

  return (
    <div className="space-y-4">
      <p className="max-w-3xl text-body text-steel-500">{t('intro')}</p>
      {error && (
        <p className="rounded-lg bg-signal/10 px-3 py-2 text-body text-signal" role="alert">
          {error}
        </p>
      )}

      <div className="grid grid-cols-1 items-start gap-4 md:grid-cols-2 2xl:grid-cols-4">
        {markets.map((market) => (
          <MarketList
            key={market}
            market={market}
            title={t('marketTitle', { market: marketName(market).toUpperCase() })}
            tags={tags.filter((x) => x.market === market)}
            editable={editable}
            locale={locale}
            onChanged={() => {
              setError(null);
              refresh();
            }}
            onError={onError}
            update={(id, body) => update.mutateAsync({ id, body })}
          />
        ))}
      </div>

      {editable && (
        <div className="flex flex-wrap items-center gap-2">
          {newMarket === null ? (
            <button type="button" className="btn-secondary btn-sm" onClick={() => setNewMarket('')}>
              <Plus className="h-4 w-4" aria-hidden />
              {t('newMarket')}
            </button>
          ) : (
            <form
              className="flex items-center gap-2"
              onSubmit={(e) => {
                e.preventDefault();
                const code = newMarket.trim().toLowerCase();
                if (!/^[a-z]{2}$/.test(code)) {
                  setError(t('newMarketInvalid'));
                  return;
                }
                setError(null);
                if (!markets.includes(code)) setExtraMarkets((m) => [...m, code]);
                setNewMarket(null);
              }}
            >
              <input
                autoFocus
                className="input w-48 font-mono"
                maxLength={2}
                placeholder={t('newMarketPlaceholder')}
                value={newMarket}
                onChange={(e) => setNewMarket(e.target.value)}
                onKeyDown={(e) => e.key === 'Escape' && setNewMarket(null)}
              />
              <button className="btn-primary btn-sm" type="submit">
                {t('done')}
              </button>
            </form>
          )}
        </div>
      )}

      {archivedTags.length > 0 && (
        <details className="card">
          <summary className="card-header cursor-pointer text-body font-medium">
            {t('archived', { count: archivedTags.length })}
          </summary>
          <ul className="card-content space-y-1">
            {archivedTags.map((tag) => (
              <li key={tag.id} className="flex items-center gap-2 text-body">
                <span className="h-3 w-3 rounded-sm" style={{ backgroundColor: tag.color }} />
                <span className="font-mono text-metadata uppercase text-steel-500">{tag.market}</span>
                <span>{tag.label}</span>
                <span className="text-metadata text-steel-500">({tag.total_leads})</span>
                {editable && (
                  <button
                    type="button"
                    className="btn-ghost btn-sm ml-auto"
                    onClick={() => update.mutate({ id: tag.id, body: { archived: false } })}
                  >
                    <RotateCcw className="h-4 w-4" aria-hidden />
                    {t('unarchive')}
                  </button>
                )}
              </li>
            ))}
          </ul>
        </details>
      )}
    </div>
  );
}

function MarketList({
  market,
  title,
  tags,
  editable,
  locale,
  onChanged,
  onError,
  update,
}: {
  market: string;
  title: string;
  tags: LeadTag[];
  editable: boolean;
  locale: string;
  onChanged: () => void;
  onError: (e: unknown) => void;
  update: (id: number, body: Parameters<typeof leadTagsApi.update>[1]) => Promise<unknown>;
}) {
  const t = useTranslations('leadTags');
  const [label, setLabel] = useState('');
  const create = useMutation({
    mutationFn: () => {
      const used = new Set(tags.map((x) => x.color));
      const color = PALETTE.find((c) => !used.has(c)) ?? '#64748b';
      return leadTagsApi.create({ market, label: label.trim(), color, domains: [] });
    },
    onSuccess: () => {
      setLabel('');
      onChanged();
    },
    onError,
  });
  const reorder = useMutation({
    mutationFn: (ids: number[]) => leadTagsApi.reorder(market, ids),
    onSuccess: onChanged,
    onError,
  });
  const move = (index: number, by: -1 | 1) => {
    const ids = tags.map((x) => x.id);
    const [id] = ids.splice(index, 1);
    if (id !== undefined) ids.splice(index + by, 0, id);
    reorder.mutate(ids);
  };

  return (
    <section className="card" data-testid={`lead-tags-${market}`}>
      <div className="card-header">
        <h2 className="text-section font-semibold">{title}</h2>
      </div>
      <ul className="card-content space-y-0.5">
        {tags.length === 0 && <li className="text-metadata text-steel-500">{t('emptyMarket')}</li>}
        {tags.map((tag, i) => (
          <TagRow
            key={tag.id}
            tag={tag}
            editable={editable}
            locale={locale}
            first={i === 0}
            last={i === tags.length - 1}
            onMove={(by) => move(i, by)}
            update={(body) => update(tag.id, body)}
          />
        ))}
      </ul>
      {editable && (
        <form
          className="card-footer"
          onSubmit={(e) => {
            e.preventDefault();
            if (label.trim()) create.mutate();
          }}
        >
          <input
            className="input"
            placeholder={t('newTag')}
            value={label}
            disabled={create.isPending}
            onChange={(e) => setLabel(e.target.value)}
          />
        </form>
      )}
    </section>
  );
}

function TagRow({
  tag,
  editable,
  locale,
  first,
  last,
  onMove,
  update,
}: {
  tag: LeadTag;
  editable: boolean;
  locale: string;
  first: boolean;
  last: boolean;
  onMove: (by: -1 | 1) => void;
  update: (body: Parameters<typeof leadTagsApi.update>[1]) => Promise<unknown>;
}) {
  const t = useTranslations('leadTags');
  const [renaming, setRenaming] = useState<string | null>(null);
  const [domain, setDomain] = useState<string | null>(null);

  const saveLabel = () => {
    const next = renaming?.trim();
    setRenaming(null);
    if (next && next !== tag.label) void update({ label: next }).catch(() => undefined);
  };
  const addDomain = () => {
    const next = domain?.trim();
    if (!next) {
      setDomain(null);
      return;
    }
    void update({ domains: [...tag.domains, next] })
      .then(() => setDomain(null))
      .catch(() => undefined);
  };

  return (
    <li className="group rounded px-1 py-1 hover:bg-steel-200/30">
      <div className="flex items-center gap-2">
        <SwatchButton
          color={tag.color}
          editable={editable}
          label={t('color')}
          onPick={(c) => void update({ color: c }).catch(() => undefined)}
        />

        {renaming !== null ? (
          <input
            autoFocus
            className="input h-7 min-w-0 flex-1 py-0"
            value={renaming}
            onChange={(e) => setRenaming(e.target.value)}
            onBlur={saveLabel}
            onKeyDown={(e) => {
              if (e.key === 'Enter') saveLabel();
              if (e.key === 'Escape') setRenaming(null);
            }}
          />
        ) : (
          <span className="flex min-w-0 flex-1 items-baseline gap-1.5">
            {editable ? (
              <button
                type="button"
                className="truncate text-left text-body text-steel-900 hover:underline"
                title={t('rename')}
                onClick={() => setRenaming(tag.label)}
              >
                {tag.label}
              </button>
            ) : (
              <span className="truncate text-body">{tag.label}</span>
            )}
            <Link
              href={`/${locale}/leads?tag=${tag.id}`}
              className="shrink-0 font-mono text-metadata text-steel-500 hover:underline"
              title={t('counts', { open: tag.open_leads, total: tag.total_leads })}
            >
              ({tag.open_leads}/{tag.total_leads})
            </Link>
          </span>
        )}

        {editable && renaming === null && (
          <span className="hidden shrink-0 items-center focus-within:flex group-focus-within:flex group-hover:flex">
            <IconButton label={t('addDomainTitle')} onClick={() => setDomain('')}>
              <Globe className="h-3.5 w-3.5" aria-hidden />
            </IconButton>
            <IconButton label={t('moveUp')} disabled={first} onClick={() => onMove(-1)}>
              <ArrowUp className="h-3.5 w-3.5" aria-hidden />
            </IconButton>
            <IconButton label={t('moveDown')} disabled={last} onClick={() => onMove(1)}>
              <ArrowDown className="h-3.5 w-3.5" aria-hidden />
            </IconButton>
            <IconButton label={t('archive')} onClick={() => void update({ archived: true }).catch(() => undefined)}>
              <Archive className="h-3.5 w-3.5" aria-hidden />
            </IconButton>
          </span>
        )}
      </div>

      {(tag.domains.length > 0 || domain !== null) && (
        <div className="ml-7 mt-1 flex flex-wrap items-center gap-1">
          {tag.domains.map((d) => (
            <span
              key={d}
              className="inline-flex items-center gap-1 rounded bg-steel-200/60 px-1.5 py-0.5 font-mono text-metadata text-steel-900"
              title={t('autoMatched', { domain: d })}
            >
              <Globe className="h-3 w-3" aria-hidden />
              {d}
              {editable && (
                <button
                  type="button"
                  className="opacity-60 hover:opacity-100"
                  aria-label={t('removeDomain', { domain: d })}
                  onClick={() =>
                    void update({ domains: tag.domains.filter((x) => x !== d) }).catch(() => undefined)
                  }
                >
                  <X className="h-3 w-3" aria-hidden />
                </button>
              )}
            </span>
          ))}
          {domain !== null && (
            <input
              autoFocus
              className="input h-7 w-44 py-0 font-mono text-metadata"
              placeholder={t('domainPlaceholder')}
              value={domain}
              onChange={(e) => setDomain(e.target.value)}
              onBlur={addDomain}
              onKeyDown={(e) => {
                if (e.key === 'Enter') addDomain();
                if (e.key === 'Escape') setDomain(null);
              }}
            />
          )}
        </div>
      )}
    </li>
  );
}

