'use client';

// Where leads come from: the list the source select offers. Renaming keeps the leads filed
// under a source; archiving takes it off the choices. Website and MiniCRM are the system's
// own and are only shown.

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Archive, ArchiveRestore, Plus } from 'lucide-react';
import { leadSourcesApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { cn } from '@/lib/utils/format';

export function LeadSourceEditor({ editable }: { editable: boolean }) {
  const t = useTranslations('sources');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const [label, setLabel] = useState('');
  const [error, setError] = useState<string | null>(null);
  const list = useQuery({ queryKey: qk.leadSources(true), queryFn: () => leadSourcesApi.list(true) });
  const done = () => {
    setError(null);
    void qc.invalidateQueries({ queryKey: ['lead-sources'] });
  };
  const onError = (e: unknown) => setError(errorMessage(e, ter, ter('unknownError')));
  const create = useMutation({
    mutationFn: () => leadSourcesApi.create({ label: label.trim() }),
    onSuccess: () => {
      setLabel('');
      done();
    },
    onError,
  });
  const update = useMutation({
    mutationFn: ({ key, body }: { key: string; body: { label?: string; archived?: boolean; position?: number } }) =>
      leadSourcesApi.update(key, body),
    onSuccess: done,
    onError,
  });
  return (
    <section className="card mt-6" data-testid="lead-sources">
      <div className="card-header">
        <h2 className="text-section font-semibold">{t('title')}</h2>
        <p className="text-metadata text-steel-500">{t('intro')}</p>
      </div>
      <div className="card-content space-y-2">
        {error && <p className="text-body text-signal" role="alert">{error}</p>}
        <ul className="divide-y divide-steel-200">
          {(list.data?.items ?? []).map((s) => (
            <li key={s.key} className={cn('flex items-center gap-2 py-1.5', s.archived_at && 'opacity-60')}>
              <input
                className="input h-8 min-w-0 flex-1 py-0"
                defaultValue={s.label}
                disabled={!editable}
                aria-label={t('label')}
                onBlur={(e) => {
                  const next = e.target.value.trim();
                  if (next && next !== s.label) update.mutate({ key: s.key, body: { label: next } });
                }}
              />
              {s.is_system && <StatusBadge tone="muted">{t('system')}</StatusBadge>}
              <span className="w-16 text-right font-mono text-metadata text-steel-500">{s.leads}</span>
              {editable && !s.is_system && (
                <button
                  type="button"
                  className="btn-ghost btn-sm"
                  title={s.archived_at ? t('restore') : t('archive')}
                  onClick={() => update.mutate({ key: s.key, body: { archived: !s.archived_at } })}
                >
                  {s.archived_at ? <ArchiveRestore className="h-4 w-4" aria-hidden /> : <Archive className="h-4 w-4" aria-hidden />}
                </button>
              )}
            </li>
          ))}
        </ul>
        {editable && (
          <form
            className="flex gap-2 border-t border-steel-200 pt-3"
            onSubmit={(e) => {
              e.preventDefault();
              if (label.trim()) create.mutate();
            }}
          >
            <input className="input h-8 min-w-0 flex-1 py-0" placeholder={t('new')} value={label} onChange={(e) => setLabel(e.target.value)} />
            <button type="submit" className="btn-secondary btn-sm" disabled={!label.trim() || create.isPending}>
              <Plus className="h-4 w-4" aria-hidden /> {t('add')}
            </button>
          </form>
        )}
      </div>
    </section>
  );
}
