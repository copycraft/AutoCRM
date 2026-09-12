'use client';

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { DetailSkeleton } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import { authApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { usePreferences } from '@/hooks/usePreferences';
import type { UserSettings } from '@/lib/api/types';

export default function PreferencesPage() {
  const t = useTranslations('preferences');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const query = usePreferences();
  const [savedTick, setSavedTick] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const save = useMutation({
    mutationFn: (body: { density?: string; page_size?: number }) =>
      authApi.savePreferences(body),
    onSuccess: (prefs) => {
      setError(null);
      setSavedTick(true);
      setTimeout(() => setSavedTick(false), 3000);
      qc.setQueryData(qk.preferences, prefs);
      void qc.invalidateQueries({ queryKey: ['partners'] });
      void qc.invalidateQueries({ queryKey: ['leads'] });
      void qc.invalidateQueries({ queryKey: ['orders'] });
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  return (
    <AppShell>
      <PageHeader title={t('title')} />
      {query.isLoading ? (
        <DetailSkeleton />
      ) : query.isError || !query.data ? (
        <ErrorState error={query.error} onRetry={() => void query.refetch()} />
      ) : (
        <PreferencesForm
          initial={query.data}
          savedTick={savedTick}
          error={error}
          pending={save.isPending}
          onSave={(density, pageSize) => save.mutate({ density, page_size: pageSize })}
        />
      )}
    </AppShell>
  );
}

function PreferencesForm({
  initial,
  savedTick,
  error,
  pending,
  onSave,
}: {
  initial: UserSettings;
  savedTick: boolean;
  error: string | null;
  pending: boolean;
  onSave: (density: string, pageSize: number) => void;
}) {
  const t = useTranslations('preferences');
  const tc = useTranslations('common');
  const [density, setDensity] = useState(initial.density);
  const [pageSize, setPageSize] = useState(String(initial.page_size));

  return (
    <section className="card">
      <div className="card-content grid grid-cols-1 gap-4 md:grid-cols-2">
        <div>
          <label className="label" htmlFor="pref-density">{t('density')}</label>
          <select
            id="pref-density"
            className="input"
            value={density}
            onChange={(e) => setDensity(e.target.value)}
          >
            <option value="comfortable">{t('densityComfortable')}</option>
            <option value="compact">{t('densityCompact')}</option>
          </select>
        </div>
        <div>
          <label className="label" htmlFor="pref-pagesize">{t('pageSize')}</label>
          <select
            id="pref-pagesize"
            className="input"
            value={pageSize}
            onChange={(e) => setPageSize(e.target.value)}
          >
            {[25, 50, 100].map((n) => (
              <option key={n} value={n}>
                {t('pageSizeRows', { n })}
              </option>
            ))}
          </select>
        </div>
      </div>
      <div className="card-footer flex-wrap justify-between gap-3">
        <span className="text-metadata text-steel-500">{savedTick ? t('saved') : ''}</span>
        <div className="flex gap-2">
          {error && (
            <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">{error}</p>
          )}
          <button
            className="btn-primary"
            disabled={pending}
            onClick={() => onSave(density, Number(pageSize))}
          >
            {pending ? tc('saving') : tc('save')}
          </button>
        </div>
      </div>
    </section>
  );
}
