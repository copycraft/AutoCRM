'use client';

// Lead tags: the coloured chip, the market names, and the picker that puts tags on a lead.
// Tags are grouped per market (the language the customer is served in), like the
// per-language sales lists the office kept in MiniCRM.

import { useMemo } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslations } from 'next-intl';
import { leadTagsApi, leadsApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { qk } from '@/lib/query/provider';
import { Chip, GroupedTagPicker } from '@/components/tags/TagKit';
import type { LeadDetail, LeadTagRef } from '@/lib/api/types';

/** The markets the app has names for, in the order the lists are shown. */
const KNOWN_MARKETS = ['hu', 'ro', 'de', 'it'];

/** "Magyar" for hu; the code in capitals for a market the app has no name for. */
export function useMarketName(): (market: string) => string {
  const t = useTranslations('leadTags');
  return (market) => (KNOWN_MARKETS.includes(market) ? t(`markets.${market}`) : market.toUpperCase());
}

/** Known markets first in a fixed order, then any others alphabetically. */
export function sortMarkets(markets: Iterable<string>): string[] {
  return Array.from(new Set(markets)).sort((a, b) => {
    const ia = KNOWN_MARKETS.indexOf(a);
    const ib = KNOWN_MARKETS.indexOf(b);
    if (ia !== -1 || ib !== -1) return (ia === -1 ? 99 : ia) - (ib === -1 ? 99 : ib);
    return a.localeCompare(b);
  });
}

export function TagChip({
  tag,
  showMarket,
  onClick,
  onRemove,
}: {
  tag: Pick<LeadTagRef, 'label' | 'color' | 'market'> & { matched_domain?: string | null };
  showMarket?: boolean;
  onClick?: () => void;
  onRemove?: () => void;
}) {
  const t = useTranslations('leadTags');
  return (
    <Chip
      label={tag.label}
      color={tag.color}
      prefix={showMarket ? tag.market : undefined}
      globe={!!tag.matched_domain}
      title={tag.matched_domain ? t('autoMatched', { domain: tag.matched_domain }) : undefined}
      onClick={onClick}
      onRemove={onRemove}
    />
  );
}

export function useLeadTags() {
  return useQuery({ queryKey: qk.leadTags(), queryFn: () => leadTagsApi.list() });
}

/** Every live lead tag in a searchable checklist, one group per market. */
export function TagPicker({
  value,
  onChange,
  disabled,
}: {
  value: number[];
  onChange: (ids: number[]) => void;
  disabled?: boolean;
}) {
  const t = useTranslations('leadTags');
  const marketName = useMarketName();
  const tags = useLeadTags();
  const groups = useMemo(() => {
    const items = tags.data?.items ?? [];
    return sortMarkets(items.map((x) => x.market)).map((m) => ({
      title: t('marketTitle', { market: marketName(m).toUpperCase() }),
      tags: items
        .filter((x) => x.market === m)
        .map((x) => ({
          id: x.id,
          label: x.label,
          color: x.color,
          keywords: `${x.domains.join(' ')} ${marketName(m)}`,
          globe: x.domains.length > 0,
        })),
    }));
    // marketName and t are stable for a given locale.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tags.data]);
  return (
    <GroupedTagPicker groups={groups} value={value} onChange={onChange} label={t('editTags')} disabled={disabled} />
  );
}

/**
 * A lead's tags on its page: the chips, and the picker that saves each change at once.
 * Auto-recognised tags show the website that put them there on hover.
 */
export function LeadTagsField({
  leadId,
  tags,
  editable,
}: {
  leadId: number;
  tags: LeadTagRef[];
  editable: boolean;
}) {
  const t = useTranslations('leadTags');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const save = useMutation({
    mutationFn: (ids: number[]) => leadsApi.setTags(leadId, ids),
    onSuccess: (res) => {
      qc.setQueryData<LeadDetail>(qk.lead(leadId), (old) => (old ? { ...old, tags: res.items } : old));
      void qc.invalidateQueries({ queryKey: ['leads'] });
      void qc.invalidateQueries({ queryKey: ['lead-tags'] });
    },
  });
  // While a save is in flight the picker shows what was asked for, so a second click
  // builds on it rather than on the tags from before.
  const ids = save.isPending && save.variables ? save.variables : tags.map((x) => x.id);
  return (
    <div className="flex flex-wrap items-center gap-2" data-testid="lead-tags">
      <span className="text-metadata text-steel-500">{t('tags')}:</span>
      {tags.length === 0 && <span className="text-metadata text-steel-500">{t('noTags')}</span>}
      {tags.map((x) => (
        <TagChip
          key={x.id}
          tag={x}
          showMarket
          onRemove={editable ? () => save.mutate(ids.filter((id) => id !== x.id)) : undefined}
        />
      ))}
      {editable && <TagPicker value={ids} onChange={(next) => save.mutate(next)} />}
      {save.isError && (
        <span className="text-metadata text-signal" role="alert">
          {errorMessage(save.error, ter, ter('unknownError'))}
        </span>
      )}
    </div>
  );
}
