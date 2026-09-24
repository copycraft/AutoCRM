// Logic audit (order lifecycle journey): the pickup board.
//  - ORD-59: the lower half lists every open order that is in the workshop
//    (design / production / meo). It must not depend on how many *other* open orders
//    exist: fetching the first 100 open orders and filtering on the client silently
//    drops in-work cars once intake orders fill the page.
//  - ORD-59: the board renders inside one app shell ("this page IS the display").
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { NextIntlClientProvider } from 'next-intl';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import messages from '@/messages/hu.json';
import { orderSummary } from './fixtures';

const api = vi.hoisted(() => ({ list: vi.fn() }));

vi.mock('@/lib/api/endpoints', () => ({ ordersApi: api }));
vi.mock('@/components/layout/AppShell', () => ({
  AppShell: ({ children }: { children: ReactNode }) => (
    <div data-testid="app-shell">{children}</div>
  ),
}));

import { PickupBoard } from '@/components/board/PickupBoard';
import BoardPage from '@/app/[locale]/board/page';

function row(id: number, stage_key: string, plate: string) {
  return { ...orderSummary, id, number: `2026-${String(id).padStart(4, '0')}`, stage_key, vehicle_plate: plate };
}

function wrap(ui: ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
  return render(
    <NextIntlClientProvider locale="hu" messages={messages} timeZone="Europe/Budapest">
      <QueryClientProvider client={qc}>{ui}</QueryClientProvider>
    </NextIntlClientProvider>,
  );
}

beforeEach(() => {
  api.list.mockReset();
  // The server behaves like the real one: `open` returns at most `limit` rows,
  // newest first — here 100 fresh intake orders fill the whole page — while a
  // stage-filtered query finds the old car in production.
  api.list.mockImplementation(async (search: Record<string, unknown>) => {
    if (search.stage === 'completed') return { items: [] };
    if (search.stage === 'production') return { items: [row(1, 'production', 'OLD-001')] };
    if (typeof search.stage === 'string') return { items: [] };
    const limit = Number(search.limit ?? 50);
    const intake = Array.from({ length: 150 }, (_, i) => row(1000 + i, 'intake', `NEW-${i}`));
    const all = [...intake, row(1, 'production', 'OLD-001')];
    return { items: all.slice(0, limit) };
  });
});

describe('pickup board', () => {
  it('lists an in-work car even when newer open orders fill the first page', async () => {
    wrap(<PickupBoard />);
    await waitFor(() => expect(screen.getByText('OLD-001')).toBeInTheDocument());
  });

  it('asks the server for the most recently completed cars, not created ones', async () => {
    wrap(<PickupBoard />);
    await waitFor(() => expect(api.list).toHaveBeenCalled());
    const completedCall = api.list.mock.calls.find(
      (call) => (call[0] as Record<string, unknown>).stage === 'completed',
    );
    expect(completedCall?.[0]).toMatchObject({ sort: '-stage_entered_at' });
  });

  it('renders the board inside exactly one app shell', async () => {
    wrap(<BoardPage />);
    await waitFor(() => expect(api.list).toHaveBeenCalled());
    expect(screen.getAllByTestId('app-shell')).toHaveLength(1);
  });
});
