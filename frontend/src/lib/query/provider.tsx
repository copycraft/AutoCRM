'use client';

import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useState, type ReactNode } from 'react';

export function QueryProvider({ children }: { children: ReactNode }) {
  const [client] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            staleTime: 30_000,
            gcTime: 5 * 60_000,
            retry: (count, err) => {
              const status = (err as { status?: number })?.status ?? 0;
              if (status === 401 || status === 403 || status === 404) return false;
              return count < 2;
            },
            refetchOnWindowFocus: false,
          },
          mutations: { retry: false },
        },
      }),
  );
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

// Canonical query keys — used for invalidation after mutations (FRONTEND_PLAN.md §14).
export const qk = {
  me: ['me'],
  users: ['users'],
  partners: (params?: unknown) => ['partners', params],
  partner: (id: number) => ['partner', id],
  leads: (params?: unknown) => ['leads', params],
  lead: (id: number) => ['lead', id],
  orders: (params?: unknown) => ['orders', params],
  order: (id: number) => ['order', id],
  orderStages: (id: number) => ['order', id, 'stages'],
  orderAudit: (id: number) => ['order', id, 'audit'],
  blockers: (params?: unknown) => ['blockers', params],
  orderBlockers: (orderId: number) => ['order', orderId, 'blockers'],
  images: (orderId: number, category?: string) => ['order', orderId, 'images', category],
  documents: (orderId: number) => ['order', orderId, 'documents'],
  emails: (params?: unknown) => ['emails', params],
  email: (id: number) => ['email', id],
  templates: ['email-templates'],
  stages: (entity?: string) => ['stage-definitions', entity],
  projectTypes: ['project-types'],
  settings: ['settings'],
  reports: (name: string, params?: unknown) => ['reports', name, params],
  adminStatus: ['admin', 'status'],
  adminJobs: (state?: string) => ['admin', 'jobs', state],
};
