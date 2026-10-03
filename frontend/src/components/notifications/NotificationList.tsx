'use client';

import { useLocale, useTranslations } from 'next-intl';
import { useRouter } from 'next/navigation';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Target } from 'lucide-react';
import { notificationsApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { cn } from '@/lib/utils/format';
import { ErrorState } from '@/components/ui/ErrorState';
import { LoadingState } from '@/components/ui/LoadingState';
import { EmptyState } from '@/components/ui/EmptyState';
import { useNotifications } from '@/components/notifications/useUnread';
import type { Notification } from '@/lib/api/types';

export function NotificationList() {
  const t = useTranslations('notifications');
  const tc = useTranslations('common');
  const locale = useLocale();
  const router = useRouter();
  const qc = useQueryClient();
  const query = useNotifications();

  const refresh = () => qc.invalidateQueries({ queryKey: qk.notifications });
  const markRead = useMutation({ mutationFn: (id: number) => notificationsApi.markRead([id]), onSuccess: refresh });
  const markAll = useMutation({ mutationFn: () => notificationsApi.markAllRead(), onSuccess: refresh });

  if (query.isError) return <ErrorState error={query.error} onRetry={() => void query.refetch()} />;
  // Also covers the moment before the session is known, when the query is not enabled yet.
  if (!query.data) return <LoadingState label={tc('loading')} />;
  const { items, unread } = query.data;

  const open = (n: Notification) => {
    if (!n.read_at) markRead.mutate(n.id);
    // The link is an app path; the locale prefix is the web app's business.
    if (n.link) router.push(`/${locale}${n.link}`);
  };

  return (
    <div className="mt-6 space-y-3">
      <div className="flex items-center justify-between">
        <p className="text-metadata text-steel-500">
          {unread} {t('unread')}
        </p>
        <button className="btn-ghost btn-sm" disabled={unread === 0 || markAll.isPending} onClick={() => markAll.mutate()}>
          {t('markAllRead')}
        </button>
      </div>
      {items.length === 0 ? (
        <EmptyState title={t('empty')} />
      ) : (
        <ul className="card divide-y divide-steel-200">
          {items.map((n) => (
            <li key={n.id}>
              <button
                onClick={() => open(n)}
                className={cn('flex w-full items-start gap-3 px-4 py-3 text-left hover:bg-panel', !n.read_at && 'bg-cold/5')}
              >
                <Target className={cn('mt-0.5 h-4 w-4 shrink-0', n.read_at ? 'text-steel-500' : 'text-cold')} aria-hidden />
                <span className="min-w-0 flex-1">
                  <span className={cn('block text-body', !n.read_at && 'font-semibold')}>{n.title}</span>
                  {n.body && <span className="block truncate text-metadata text-steel-500">{n.body}</span>}
                  <span className="block text-metadata text-steel-500">
                    {new Date(n.created_at).toLocaleString('hu-HU')}
                  </span>
                </span>
                {!n.read_at && <span className="mt-1.5 h-2 w-2 shrink-0 rounded-full bg-cold" aria-label={t('unread')} />}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
