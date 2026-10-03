// Számlázó: one desk for every invoice, storno and díjbekérő, plus a create
// panel that opens the per-order sections for a picked order.
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { renderPage } from './harness';
import { BillingPage } from '@/components/billing/BillingPage';

vi.mock('@/lib/api/client', () => import('./client-mock'));
vi.mock('@/components/layout/AppShell', () => ({
  AppShell: ({ children }: { children: ReactNode }) => (
    <div data-testid="app-shell">{children}</div>
  ),
}));

import { AppShell } from '@/components/layout/AppShell';

describe('Számlázó page', () => {
  it('lists every invoice and storno with its order and partner', async () => {
    renderPage(<BillingPage />);
    await screen.findByText('AT2026-0001');
    expect(screen.getByText('AT2026-0004')).toBeInTheDocument();
    // The storno reads as one, with its negative total.
    expect(screen.getAllByText('Sztornó').length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText(/#MC-1001/).length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText(/Müller Kühltransporte/)[0]).toBeInTheDocument();
  });

  it('the proforma tab offers invoicing from the proforma order', async () => {
    renderPage(<BillingPage />);
    await screen.findByText('AT2026-0001');
    fireEvent.click(screen.getByRole('tab', { name: 'Díjbekérők' }));
    await screen.findByText('DB2026-0001');
    fireEvent.click(screen.getByRole('button', { name: 'Számla kiállítása' }));
    // The create panel opens the picked order's sections.
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Díjbekérő készítése' })).toBeInTheDocument(),
    );
  });

  it('picking an order from search opens its invoice section', async () => {
    renderPage(<BillingPage />);
    await screen.findByText('AT2026-0001');
    const search = screen.getByLabelText('Melyik munkára?');
    fireEvent.change(search, { target: { value: 'MC-10' } });
    const pick = await screen.findByRole('button', { name: /MC-1001/ });
    fireEvent.click(pick);
    // The picked order's own sections open in the create panel.
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Díjbekérő készítése' })).toBeInTheDocument(),
    );
  });

  it('the kind filter narrows the history', async () => {
    renderPage(<BillingPage />);
    await screen.findByText('AT2026-0001');
    fireEvent.change(screen.getByLabelText('Típus'), { target: { value: 'storno' } });
    await waitFor(() => expect(screen.queryByText('AT2026-0001')).toBeNull());
    expect(await screen.findByText('AT2026-0004')).toBeInTheDocument();
  });

  it('renders inside exactly one app shell', async () => {
    renderPage(
      <AppShell>
        <BillingPage />
      </AppShell>,
    );
    await screen.findByText('AT2026-0001');
    expect(screen.getAllByTestId('app-shell')).toHaveLength(1);
  });
});
