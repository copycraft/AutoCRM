'use client';

import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslations } from 'next-intl';
import { History, RotateCcw } from 'lucide-react';
import { yardLayoutApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { useToast } from '@/components/ui/Toasts';
import { DetailSkeleton } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import { ConfirmDialog } from '@/components/ui/ConfirmDialog';
import { DateDisplay } from '@/components/ui/DateDisplay';
import type { LayoutRevision } from '@/lib/api/types';

export function YardRevisions() {
  const t = useTranslations('yard3d');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const { toast } = useToast();
  const [revertTarget, setRevertTarget] = useState<LayoutRevision | null>(null);

  const revisionsQ = useQuery({
    queryKey: qk.yardRevisions,
    queryFn: () => yardLayoutApi.revisions(50),
  });

  const revert = useMutation({
    mutationFn: (body: { revision_id: number; note: string }) => yardLayoutApi.revert(body),
    onSuccess: () => {
      setRevertTarget(null);
      void qc.invalidateQueries({ queryKey: ['yard'] });
      toast('info', t('revert'));
    },
    onError: (e) => toast('error', errorMessage(e, ter, ter('unknownError'))),
  });

  if (revisionsQ.isPending) return <DetailSkeleton />;
  if (revisionsQ.isError) return <ErrorState error={revisionsQ.error} onRetry={() => void revisionsQ.refetch()} />;

  const items = revisionsQ.data?.items ?? [];

  return (
    <section className="card">
      <div className="card-header">
        <h2 className="text-section font-semibold">{t('revisions')}</h2>
      </div>
      <div className="card-content">
        {items.length === 0 ? (
          <p className="text-metadata text-steel-500">{t('empty')}</p>
        ) : (
          <ul className="divide-y divide-steel-200">
            {items.map((r) => (
              <li key={r.id} className="flex items-center justify-between gap-3 py-2">
                <div className="min-w-0">
                  <p className="text-body font-medium">
                    <DateDisplay withTime value={r.created_at} />
                    {r.created_by_name ? ` · ${r.created_by_name}` : ''}
                  </p>
                  {r.note && <p className="truncate text-metadata text-steel-500">{r.note}</p>}
                </div>
                <button
                  type="button"
                  className="btn-ghost btn-sm shrink-0"
                  onClick={() => setRevertTarget(r)}
                >
                  <RotateCcw className="h-4 w-4" aria-hidden />
                  {t('revert')}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
      <ConfirmDialog
        open={revertTarget != null}
        title={t('revert')}
        body={t('revertConfirm')}
        confirmLabel={t('revert')}
        onConfirm={() => {
          if (revertTarget) {
            revert.mutate({ revision_id: revertTarget.id, note: '' });
          }
        }}
        onClose={() => setRevertTarget(null)}
        busy={revert.isPending}
      />
    </section>
  );
}
