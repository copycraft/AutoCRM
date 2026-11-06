'use client';

// Hirdetések: a job listing run as ads on Facebook, Instagram, Google… Each ad is filed
// here with what it cost and gets its own tracked link to paste into the ad; the table then
// says which ad brought how many visits and applicants, and what an applicant cost. Our own
// numbers: nothing is sent to the ad platforms. Applicants who came some other way are
// sorted by channel below.

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { hrApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { parseMajorToMinor } from '@/lib/utils/format';
import { useToast } from '@/components/ui/Toasts';
import { Money } from '@/components/ui/Money';
import { LoadingState } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import type { JobAdView } from '@/lib/api/types';

const PLATFORMS = ['facebook', 'instagram', 'google', 'tiktok', 'other'] as const;
type Platform = (typeof PLATFORMS)[number];

export const jobAdsKey = (jobId: number) => ['hr', 'jobs', jobId, 'ads'];

export function JobAdsSection({ jobId }: { jobId: number }) {
  const t = useTranslations('jobAds');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const toast = useToast();
  const [adding, setAdding] = useState(false);
  const [showArchived, setShowArchived] = useState(false);
  const query = useQuery({ queryKey: jobAdsKey(jobId), queryFn: () => hrApi.jobAds(jobId) });

  const refresh = () => void qc.invalidateQueries({ queryKey: jobAdsKey(jobId) });
  const archive = useMutation({
    mutationFn: (ad: JobAdView) =>
      hrApi.updateJobAd(ad.id, {
        platform: ad.platform,
        name: ad.name,
        headline: ad.headline,
        body: ad.body,
        spend_minor: ad.spend_minor,
        starts_on: ad.starts_on,
        ends_on: ad.ends_on,
        archived: ad.archived_at == null,
      }),
    onSuccess: refresh,
    onError: (e) => toast.error(errorMessage(e, ter, ter('unknownError'))),
  });

  const copy = async (link: string) => {
    try {
      await navigator.clipboard.writeText(link);
      toast.success(t('linkCopied'));
    } catch {
      toast.info(link);
    }
  };

  if (query.isLoading) return <LoadingState />;
  if (query.isError || !query.data) return <ErrorState error={query.error} onRetry={() => void query.refetch()} />;
  const ads = query.data.ads.filter((a) => showArchived || a.archived_at == null);
  const archivedCount = query.data.ads.filter((a) => a.archived_at != null).length;

  return (
    <section className="space-y-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h3 className="text-section font-semibold">{t('title')}</h3>
          <p className="max-w-2xl text-metadata text-steel-500">{t('hint')}</p>
        </div>
        <div className="flex gap-2">
          {archivedCount > 0 && (
            <button type="button" className="btn-ghost btn-sm" onClick={() => setShowArchived((v) => !v)}>
              {showArchived ? t('hideArchived') : t('showArchived', { count: archivedCount })}
            </button>
          )}
          <button type="button" className="btn-secondary btn-sm" onClick={() => setAdding((v) => !v)}>
            {adding ? t('cancel') : t('add')}
          </button>
        </div>
      </div>

      {adding && <AdForm jobId={jobId} onSaved={() => { setAdding(false); refresh(); }} />}

      {ads.length === 0 ? (
        <p className="text-body text-steel-500">{t('none')}</p>
      ) : (
        <div className="card overflow-x-auto">
          <table className="w-full text-body">
            <thead>
              <tr className="border-b border-steel-200 text-left text-metadata text-steel-500">
                <th className="px-4 py-2 font-medium">{t('ad')}</th>
                <th className="px-2 py-2 text-right font-medium">{t('visits')}</th>
                <th className="px-2 py-2 text-right font-medium">{t('applications')}</th>
                <th className="px-2 py-2 text-right font-medium">{t('conversion')}</th>
                <th className="px-2 py-2 text-right font-medium">{t('spend')}</th>
                <th className="px-2 py-2 text-right font-medium">{t('perApplication')}</th>
                <th className="px-4 py-2"><span className="sr-only">{t('actions')}</span></th>
              </tr>
            </thead>
            <tbody>
              {ads.map((a) => (
                <tr key={a.id} className={`border-b border-steel-200 last:border-0 ${a.archived_at ? 'opacity-60' : ''}`}>
                  <td className="px-4 py-2">
                    <p className="font-medium">{a.name}</p>
                    <p className="text-metadata text-steel-500">
                      {t(`platforms.${a.platform as Platform}`)}
                      {a.starts_on ? ` · ${a.starts_on}${a.ends_on ? ` – ${a.ends_on}` : ''}` : ''}
                    </p>
                  </td>
                  <td className="px-2 py-2 text-right font-mono">{a.visits}</td>
                  <td className="px-2 py-2 text-right font-mono font-semibold">{a.applications}</td>
                  <td className="px-2 py-2 text-right font-mono">
                    {a.visits > 0 ? `${((a.applications / a.visits) * 100).toFixed(1)}%` : '—'}
                  </td>
                  <td className="px-2 py-2 text-right"><Money minor={a.spend_minor} currency="HUF" /></td>
                  <td className="px-2 py-2 text-right"><Money minor={a.cost_per_application_minor} currency="HUF" /></td>
                  <td className="whitespace-nowrap px-4 py-2 text-right">
                    <button type="button" className="btn-secondary btn-sm" onClick={() => void copy(a.link)}>
                      {t('copyLink')}
                    </button>
                    <button type="button" className="btn-ghost btn-sm" onClick={() => archive.mutate(a)}>
                      {a.archived_at ? t('unarchive') : t('archive')}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {query.data.other.length > 0 && (
        <div>
          <p className="text-metadata font-medium text-steel-500">{t('otherTitle')}</p>
          <ul className="mt-1 flex flex-wrap gap-2">
            {query.data.other.map((c) => (
              <li key={c.channel} className="rounded-lg bg-steel-200/50 px-3 py-1 text-metadata">
                {t(`channels.${c.channel}`)}: {t('otherCounts', { visits: c.visits, applications: c.applications })}
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}

function AdForm({ jobId, onSaved }: { jobId: number; onSaved: () => void }) {
  const t = useTranslations('jobAds');
  const ter = useTranslations('errors');
  const [platform, setPlatform] = useState<Platform>('facebook');
  const [name, setName] = useState('');
  const [headline, setHeadline] = useState('');
  const [body, setBody] = useState('');
  const [spend, setSpend] = useState('');
  const [startsOn, setStartsOn] = useState('');
  const [endsOn, setEndsOn] = useState('');
  const [error, setError] = useState<string | null>(null);

  const save = useMutation({
    mutationFn: () =>
      hrApi.createJobAd(jobId, {
        platform,
        name: name.trim(),
        headline: headline.trim() || null,
        body: body.trim() || null,
        spend_minor: spend.trim() ? parseMajorToMinor(spend.trim()) : null,
        starts_on: startsOn || null,
        ends_on: endsOn || null,
      }),
    onSuccess: onSaved,
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  return (
    <form
      className="card space-y-3 p-4"
      onSubmit={(e) => {
        e.preventDefault();
        setError(null);
        if (name.trim()) save.mutate();
      }}
    >
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-[12rem_1fr]">
        <div>
          <label className="label" htmlFor="ad-platform">{t('platform')}</label>
          <select id="ad-platform" className="input" value={platform} onChange={(e) => setPlatform(e.target.value as Platform)}>
            {PLATFORMS.map((p) => (
              <option key={p} value={p}>
                {t(`platforms.${p}`)}
              </option>
            ))}
          </select>
        </div>
        <div>
          <label className="label" htmlFor="ad-name">{t('name')}</label>
          <input id="ad-name" className="input" value={name} placeholder={t('namePlaceholder')} onChange={(e) => setName(e.target.value)} />
        </div>
      </div>
      <div>
        <label className="label" htmlFor="ad-headline">{t('headline')}</label>
        <input id="ad-headline" className="input" value={headline} onChange={(e) => setHeadline(e.target.value)} />
      </div>
      <div>
        <label className="label" htmlFor="ad-body">{t('body')}</label>
        <textarea id="ad-body" className="input min-h-[5rem]" value={body} onChange={(e) => setBody(e.target.value)} />
      </div>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
        <div>
          <label className="label" htmlFor="ad-spend">{t('spendHuf')}</label>
          <input id="ad-spend" className="input font-mono" inputMode="decimal" value={spend} onChange={(e) => setSpend(e.target.value)} />
        </div>
        <div>
          <label className="label" htmlFor="ad-start">{t('startsOn')}</label>
          <input id="ad-start" type="date" className="input" value={startsOn} onChange={(e) => setStartsOn(e.target.value)} />
        </div>
        <div>
          <label className="label" htmlFor="ad-end">{t('endsOn')}</label>
          <input id="ad-end" type="date" className="input" value={endsOn} onChange={(e) => setEndsOn(e.target.value)} />
        </div>
      </div>
      <p className="text-metadata text-steel-500">{t('formHint')}</p>
      {error && (
        <p className="text-body text-signal" role="alert">
          {error}
        </p>
      )}
      <div className="flex justify-end">
        <button type="submit" className="btn-primary btn-sm" disabled={!name.trim() || save.isPending}>
          {t('save')}
        </button>
      </div>
    </form>
  );
}
