// Partner picker (N7/SALES-17): customer pickers must not offer suppliers.
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, screen, waitFor } from '@testing-library/react';
import { renderPage } from './harness';

const api = vi.hoisted(() => ({ list: vi.fn() }));

vi.mock('@/lib/api/endpoints', () => ({ partnersApi: api }));

import { PartnerPicker } from '@/components/forms/PartnerPicker';

beforeEach(() => {
  api.list.mockReset();
  api.list.mockResolvedValue({ items: [] });
});

async function openPicker() {
  renderPage(<PartnerPicker value={null} onChange={() => {}} label="Partner" role="customer" />);
  const box = screen.getByRole('combobox');
  fireEvent.focus(box);
  fireEvent.change(box, { target: { value: 'Aut' } });
}

describe('PartnerPicker role filter', () => {
  it('restricts customer pickers to customers server-side', async () => {
    await openPicker();
    // The picker queries on open and again once the typed query settles.
    await waitFor(() => {
      const hit = api.list.mock.calls.find(
        (call) => (call[0] as Record<string, unknown> | undefined)?.q === 'Aut',
      );
      expect(hit?.[0]).toMatchObject({ role: 'customer' });
    });
  });

  it('sends no role filter without the prop', async () => {
    renderPage(<PartnerPicker value={null} onChange={() => {}} label="Partner" />);
    const box = screen.getByRole('combobox');
    fireEvent.focus(box);
    fireEvent.change(box, { target: { value: 'Aut' } });
    await waitFor(() => expect(api.list).toHaveBeenCalled());
    const first = api.list.mock.calls[0];
    expect(first).toBeDefined();
    const arg = (first as unknown[])[0] as Record<string, unknown>;
    expect(arg.role).toBeUndefined();
  });
});
