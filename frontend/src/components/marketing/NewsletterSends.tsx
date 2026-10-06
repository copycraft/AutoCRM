'use client';

// Tracked newsletter sends: what went (or is going) out, to how many readers, and how
// many opened it, clicked a link or left. A send scheduled for later can be cancelled
// until it goes.

import { useTranslations } from 'next-intl';
import { useMutation, useQueries, useQuery, useQueryClient } from '@tanstack/react-query';
import { Ban, Clock } from 'lucide-react';
import { newsletterApi } from '@/lib/api/endpoints';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { ErrorState } from '@/components/ui/ErrorState';
import { EmptyState } from '@/components/ui/EmptyState';

/** A share as a whole percent; "—" with nothing to divide by. */
export function pct(part: number, whole: number): string {
  return whole > 0 ? `${Math.round((part / whole) * 100)}%` : '—';
}

export function NewsletterSends({ editable }: { editable: boolean }) {
  const t = useTranslations('marketing');
  const qc = useQueryClient();
  const sends = useQuery({ queryKey: ['newsletter-sends'], queryFn: () => newsletterApi.sends() });
  const rows = sends.data?.items ?? [];
  const stats = useQueries({
    queries: rows.map((s) => ({
      queryKey: ['newsletter-send-stats', s.id],
      queryFn: () => newsletterApi.sendStats(s.id),
      enabled: !s.cancelled_at,
      staleTime: 60_000,
    })),
  });
  const cancel = useMutation({
    mutationFn: (id: number) => newsletterApi.cancelSend(id),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ['newsletter-sends'] }),
  });

  if (sends.isError) return <ErrorState error={sends.error} onRetry={() => void sends.refetch()} />;
  if (!sends.isPending && rows.length === 0) {
    return <EmptyState title={t('sendsEmpty')} hint={t('sendsEmptyHint')} />;
  }
  const now = Date.now();

  return (
    <div className="card overflow-x-auto" data-testid="newsletter-sends">
      <table className="w-full text-body">
        <thead className="border-b border-steel-200 text-left text-metadata text-steel-500">
          <tr>
            <th className="px-3 py-2">{t('sendSubject')}</th>
            <th className="px-3 py-2">{t('sendWhen')}</th>
            <th className="px-3 py-2 text-right">{t('sendRecipients')}</th>
            <th className="px-3 py-2 text-right">{t('sendDelivered')}</th>
            <th className="px-3 py-2 text-right">{t('sendOpened')}</th>
            <th className="px-3 py-2 text-right">{t('sendClicked')}</th>
            <th className="px-3 py-2 text-right">{t('sendUnsubscribed')}</th>
            {editable && <th className="px-3 py-2" />}
          </tr>
        </thead>
        <tbody>
          {rows.map((s, i) => {
            const st = stats[i]?.data;
            const scheduled = !s.cancelled_at && new Date(s.send_at).getTime() > now;
            return (
              <tr key={s.id} className="border-b border-steel-200 last:border-0">
                <td className="px-3 py-2 font-medium">{s.subject}</td>
                <td className="px-3 py-2 text-metadata">
                  <span className="flex flex-wrap items-center gap-1.5">
                    <DateDisplay withTime value={s.send_at} />
                    {s.cancelled_at ? (
                      <StatusBadge tone="muted">{t('sendCancelled')}</StatusBadge>
                    ) : scheduled ? (
                      <StatusBadge tone="cold">
                        <Clock className="mr-1 inline h-3 w-3" aria-hidden />
                        {t('sendScheduled')}
                      </StatusBadge>
                    ) : null}
                  </span>
                </td>
                <td className="px-3 py-2 text-right font-mono">{st?.recipients ?? s.recipients}</td>
                <td className="px-3 py-2 text-right font-mono">{st ? st.sent : '—'}</td>
                <td className="px-3 py-2 text-right font-mono" title={st ? t('sendOpens', { count: st.opens }) : undefined}>
                  {st ? `${st.opened} (${pct(st.opened, st.sent)})` : '—'}
                </td>
                <td className="px-3 py-2 text-right font-mono" title={st ? t('sendClicks', { count: st.clicks }) : undefined}>
                  {st ? `${st.clicked} (${pct(st.clicked, st.sent)})` : '—'}
                </td>
                <td className="px-3 py-2 text-right font-mono">{st ? st.unsubscribed : '—'}</td>
                {editable && (
                  <td className="px-3 py-2 text-right">
                    {scheduled && (
                      <button
                        type="button"
                        className="btn-ghost btn-sm"
                        disabled={cancel.isPending}
                        onClick={() => cancel.mutate(s.id)}
                      >
                        <Ban className="h-4 w-4" aria-hidden />
                        {t('sendCancel')}
                      </button>
                    )}
                  </td>
                )}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
