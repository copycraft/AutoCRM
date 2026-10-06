'use client';

import { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { skipToken, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { DetailSkeleton } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import { LeadForm, leadCreateBody, type LeadFormValues } from '@/components/forms/LeadForm';
import { leadsApi, partnersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { rememberLastUsed } from '@/hooks/useLastUsed';
import { useAuth } from '@/lib/auth/context';
import { TagChip, TagPicker, useLeadTags } from '@/components/leads/LeadTags';

export default function NewLeadPage() {
  const t = useTranslations('leads');
  const tc = useTranslations('common');
  const locale = useLocale();
  const router = useRouter();
  const qc = useQueryClient();
  const { user } = useAuth();
  const [cloneId, setCloneId] = useState<number | null>(null);
  const tt = useTranslations('leadTags');
  const allTags = useLeadTags();
  // null until touched: a clone starts with the original's tags.
  const [pickedTags, setPickedTags] = useState<number[] | null>(null);

  useEffect(() => {
    try {
      const raw = new URLSearchParams(window.location.search).get('clone');
      const n = raw ? Number(raw) : NaN;
      if (Number.isFinite(n) && n > 0) setCloneId(Math.floor(n));
    } catch {
      /* non-browser — plain empty form */
    }
  }, []);

  const cloneQuery = useQuery({
    queryKey: cloneId !== null ? qk.lead(cloneId) : ['lead', 'none'],
    queryFn: cloneId === null ? skipToken : () => leadsApi.get(cloneId),
  });
  const tagIds =
    pickedTags ??
    (cloneQuery.data?.tags ?? [])
      .map((x) => x.id)
      .filter((id) => allTags.data?.items.some((x) => x.id === id));
  const clonePartnerId = cloneQuery.data?.lead.partner_id ?? null;
  const clonePartnerQuery = useQuery({
    queryKey: clonePartnerId !== null ? qk.partner(clonePartnerId) : ['partner', 'none'],
    queryFn: clonePartnerId === null ? skipToken : () => partnersApi.get(clonePartnerId),
  });

  const create = useMutation({
    mutationFn: (v: LeadFormValues) => leadsApi.create({ ...leadCreateBody(v, user?.id), tag_ids: tagIds }),
    onSuccess: (lead) => {
      void qc.invalidateQueries({ queryKey: ['leads'] });
      router.replace(`/${locale}/leads/${lead.id}`);
    },
  });

  const cloning = cloneId !== null;
  const cloneReady = !cloning || (cloneQuery.data && (clonePartnerId === null || clonePartnerQuery.data));

  return (
    <AppShell>
      <PageHeader
        title={t('newLead')}
        subtitle={cloning && cloneQuery.data ? t('cloneOf', { title: cloneQuery.data.lead.title }) : undefined}
      />
      {cloning && cloneQuery.isLoading ? (
        <DetailSkeleton />
      ) : cloning && (cloneQuery.isError || !cloneQuery.data) ? (
        <ErrorState error={cloneQuery.error} onRetry={() => void cloneQuery.refetch()} />
      ) : !cloneReady ? (
        <DetailSkeleton />
      ) : (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-metadata text-steel-500">{tt('tags')}:</span>
            {(allTags.data?.items ?? [])
              .filter((x) => tagIds.includes(x.id))
              .map((x) => (
                <TagChip
                  key={x.id}
                  tag={x}
                  showMarket
                  onRemove={() => setPickedTags(tagIds.filter((id) => id !== x.id))}
                />
              ))}
            <TagPicker value={tagIds} onChange={setPickedTags} />
          </div>
          <LeadForm
            key={cloneId ?? 'new'}
            initial={cloneQuery.data?.lead}
            initialPartner={
              clonePartnerQuery.data
                ? { id: clonePartnerQuery.data.partner.id, name: clonePartnerQuery.data.partner.name }
                : null
            }
            draftKey={cloning ? undefined : 'lead-new'}
            submitLabel={tc('create')}
            onSubmit={(v) => {
              if (v.assigned_to !== null) rememberLastUsed('assignee', String(v.assigned_to));
              return create.mutateAsync(v).then(() => undefined);
            }}
          />
        </>
      )}
    </AppShell>
  );
}
