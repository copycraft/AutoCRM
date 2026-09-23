'use client';

import { QueryClient, QueryClientProvider, keepPreviousData } from '@tanstack/react-query';
import { useState, type ReactNode } from 'react';
import { ApiError } from '@/lib/api/errors';

export function QueryProvider({ children }: { children: ReactNode }) {
  const [client] = useState(
    () =>
      new QueryClient({
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
      }),
  );
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

// Canonical query keys — used for invalidation after mutations (FRONTEND_PLAN.md §14).
export const qk = {
  me: ['me'],
  preferences: ['preferences'],
  users: ['users'],
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
  reports: (name: string, params?: unknown) => ['reports', name, params],
  inspections: (params?: unknown) => ['inspections', params],
  inspection: (id: number) => ['inspection', id],
  inspectionComparison: (id: number) => ['inspection', id, 'comparison'],
  inspectionTemplates: (key?: string | number) => ['inspection-templates', key],
  adminStatus: ['admin', 'status'],
  adminJobs: (state?: string) => ['admin', 'jobs', state],
};
