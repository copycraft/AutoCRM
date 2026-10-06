'use client';

// Why leads are lost: the short list offered when a lead moves to Elveszett. Renaming
// keeps the leads already filed under a reason; archiving takes it off the choices.

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Archive, ArchiveRestore, Plus } from 'lucide-react';
import { leadsApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { cn } from '@/lib/utils/format';

export function LostReasonEditor({ editable }: { editable: boolean }) {
  const t = useTranslations('leads');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const [label, setLabel] = useState('');
  const [error, setError] = useState<string | null>(null);
  const reasons = useQuery({ queryKey: ['lost-reasons', 'all'], queryFn: () => leadsApi.lostReasons(true) });

  const done = () => {
    setError(null);
    void qc.invalidateQueries({ queryKey: ['lost-reasons'] });
  };
  const onError = (e: unknown) => setError(errorMessage(e, ter, ter('unknownError')));
  const create = useMutation({
    mutationFn: () => leadsApi.createLostReason({ label: label.trim() }),
    onSuccess: () => {
      setLabel('');
      done();
    },
    onError,
  });
  const update = useMutation({
    mutationFn: ({ id, label, archived }: { id: number; label: string; archived: boolean }) =>
      leadsApi.updateLostReason(id, { label, archived }),
    onSuccess: done,
    onError,
  });

  return (
    <section className="card mt-6" data-testid="lost-reasons">
      <div className="card-header">
        <h2 className="text-section font-semibold">{t('lostReasonsTitle')}</h2>
        <p className="text-metadata text-steel-500">{t('lostReasonsIntro')}</p>
      </div>
      <div className="card-content space-y-2">
        {error && <p className="text-body text-signal" role="alert">{error}</p>}
        <ul className="divide-y divide-steel-200">
          {(reasons.data?.items ?? []).map((r) => (
            <li key={r.id} className={cn('flex items-center gap-2 py-1.5', r.archived_at && 'opacity-60')}>
              <input
                className="input h-8 min-w-0 flex-1 py-0"
                defaultValue={r.label}
                disabled={!editable}
                aria-label={t('lostReason')}
                onBlur={(e) => {
                  const next = e.target.value.trim();
                  if (next && next !== r.label) update.mutate({ id: r.id, label: next, archived: !!r.archived_at });
                }}
              />
              <span className="w-16 text-right font-mono text-metadata text-steel-500">{r.leads}</span>
              {editable && (
                <button
                  type="button"
                  className="btn-ghost btn-sm"
                  title={r.archived_at ? t('restore') : t('archive')}
                  onClick={() => update.mutate({ id: r.id, label: r.label, archived: !r.archived_at })}
                >
                  {r.archived_at ? <ArchiveRestore className="h-4 w-4" aria-hidden /> : <Archive className="h-4 w-4" aria-hidden />}
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
            <input
              className="input h-8 min-w-0 flex-1 py-0"
              placeholder={t('lostReasonNew')}
              value={label}
              onChange={(e) => setLabel(e.target.value)}
            />
            <button type="submit" className="btn-secondary btn-sm" disabled={!label.trim() || create.isPending}>
              <Plus className="h-4 w-4" aria-hidden />
              {t('add')}
            </button>
          </form>
        )}
      </div>
    </section>
  );
}
