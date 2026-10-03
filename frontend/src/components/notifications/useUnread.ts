'use client';

import { useQuery } from '@tanstack/react-query';
import { notificationsApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { useAuth } from '@/lib/auth/context';

/** How often the web app asks whether something new came in. */
export const NOTIFICATION_POLL_MS = 30_000;

/** The signed-in user's notifications, refreshed on an interval. */
export function useNotifications() {
  const { isAuthenticated } = useAuth();
  return useQuery({
    queryKey: qk.notifications,
    queryFn: () => notificationsApi.list({ limit: 50 }),
    enabled: isAuthenticated,
    refetchInterval: NOTIFICATION_POLL_MS,
    refetchIntervalInBackground: false,
  });
}
