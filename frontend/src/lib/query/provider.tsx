'use client';

import {
  MutationCache,
  QueryCache,
  QueryClient,
  QueryClientProvider,
  keepPreviousData,
} from '@tanstack/react-query';
import { useState, type ReactNode } from 'react';
import { ApiError } from '@/lib/api/errors';

export function QueryProvider({ children }: { children: ReactNode }) {
  const [client] = useState(() => {
    // docs/history/FRONTEND_PLAN.md §14: "401 returns to login". The AppShell redirects when the cached
    // `me` is empty, so a 401 from any request (session expired, revoked, or the account
    // deactivated) empties it; otherwise only a reload would notice.
    const signOutOn401 = (err: unknown) => {
      if (err instanceof ApiError && err.status === 401) qc.setQueryData(qk.me, null);
    };
    const qc: QueryClient = new QueryClient({
        queryCache: new QueryCache({ onError: signOutOn401 }),
        mutationCache: new MutationCache({ onError: signOutOn401 }),
        defaultOptions: {
          queries: {
            staleTime: 30_000,
            gcTime: 5 * 60_000,
            // Paginating or refiltering keeps the old rows on screen until the new ones
            // arrive, instead of flashing a skeleton on every page turn.
            placeholderData: keepPreviousData,
            retry: (count, err) => {
              const status = err instanceof ApiError ? err.status : 0;
              if (status === 401 || status === 403 || status === 404) return false;
              return count < 2;
            },
            refetchOnWindowFocus: false,
          },
          mutations: { retry: false },
        },
      });
    return qc;
  });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

// Canonical query keys — used for invalidation after mutations (docs/history/FRONTEND_PLAN.md §14).
export const qk = {
  me: ['me'],
  preferences: ['preferences'],
  users: ['users'],
  employees: (params?: unknown) => ['employees', params],
  partners: (params?: unknown) => ['partners', params],
  partner: (id: number) => ['partner', id],
  leads: (params?: unknown) => ['leads', params],
  lead: (id: number) => ['lead', id],
  orders: (params?: unknown) => ['orders', params],
  order: (id: number) => ['order', id],
  orderStages: (id: number) => ['order', id, 'stages'],
  orderTransitions: (id: number) => ['order', id, 'transitions'],
  leadTransitions: (id: number) => ['lead', id, 'transitions'],
  orderAudit: (id: number) => ['order', id, 'audit'],
  orderNotes: (id: number) => ['order', id, 'notes'],
  rawImport: (entity: 'partner' | 'lead' | 'order', id: number) => ['raw-import', entity, id],
  blockers: (params?: unknown) => ['blockers', params],
  orderBlockers: (orderId: number) => ['order', orderId, 'blockers'],
  images: (orderId: number, category?: string) => ['order', orderId, 'images', category],
  documents: (orderId: number) => ['order', orderId, 'documents'],
  invoices: (orderId: number) => ['order', orderId, 'invoices'],
  invoicesAll: (params?: unknown) => ['invoices', params],
  proformasAll: ['proformas'],
  invoice: (id: number) => ['invoice', id],
  invoiceChain: (id: number) => ['invoice', id, 'chain'],
  proformas: (orderId: number) => ['order', orderId, 'proformas'],
  emails: (params?: unknown) => ['emails', params],
  email: (id: number) => ['email', id],
  search: (q: string) => ['search', q],
  tasksMine: ['tasks', 'mine'],
  tasksFor: (entity: string, id: number) => ['tasks', entity, id],
  templates: ['email-templates'],
  stages: (entity?: string) => ['stage-definitions', entity],
  projectTypes: ['project-types'],
  settings: ['settings'],
  lookups: ['lookups'],
  reports: (name: string, params?: unknown) => ['reports', name, params],
  inspections: (params?: unknown) => ['inspections', params],
  inspection: (id: number) => ['inspection', id],
  inspectionComparison: (id: number) => ['inspection', id, 'comparison'],
  inspectionTemplates: (projectTypeId?: number | null, kind?: string) => [
    'inspection-templates',
    projectTypeId ?? null,
    kind,
  ],
  adminStatus: ['admin', 'status'],
  adminJobs: (state?: string) => ['admin', 'jobs', state],
};
