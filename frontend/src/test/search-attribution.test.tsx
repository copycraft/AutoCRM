// Search results are built the same way for the box and the palette, and the report says
// where leads come from.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { screen } from '@testing-library/react';
import { renderPage } from './harness';
import { overrides, resetOverrides } from './client-mock';
import { buildSearchHits } from '@/components/search/searchHits';
import { LeadSourcesReport, winRate } from '@/components/reports/LeadSourcesReport';

vi.mock('@/lib/api/client', () => import('./client-mock'));

afterEach(() => resetOverrides());

const labels = {
  orders: 'Munkák',
  partners: 'Partnerek',
  leads: 'Leadek',
  contacts: 'Kapcsolattartók',
  emails: 'E-mailek',
  employees: 'Munkatársak',
};

describe('search hits', () => {
  it('lists every group that has results, in order, with links to where each opens', () => {
    const { flat, groups } = buildSearchHits(
      {
        orders: [{ id: 1, number: '2026-0001', title: 'Hűtős Sprinter', plate: 'ABC-123', stage_label: 'Gyártás' }],
        partners: [],
        leads: [],
        contacts: [{ id: 9, partner_id: 4, name: 'Szabó Ilona', partner_name: 'Telefon Kft.', email: null, phone: '+36 20 111 2222' }],
        emails: [{ id: 3, subject: 'Festék', to_address: 'a@b.hu', status: 'sent', queued_at: '2026-01-01T00:00:00Z' }],
        employees: [{ id: 2, full_name: 'Kiss Péter', email: null, company_phone: null, archived: true }],
      },
      'hu',
      labels,
    );
    expect(groups.map((g) => g.label)).toEqual(['Munkák', 'Kapcsolattartók', 'E-mailek', 'Munkatársak']);
    expect(groups.map((g) => [g.from, g.to])).toEqual([[0, 1], [1, 2], [2, 3], [3, 4]]);
    expect(flat.map((h) => h.href)).toEqual([
      '/hu/orders/1',
      // A contact opens the company; an employee opens the HR page already searched.
      '/hu/partners/4',
      '/hu/emails/3',
      '/hu/hr?q=Kiss%20P%C3%A9ter',
    ]);
    expect(flat[1]!.sub).toBe('Telefon Kft. · +36 20 111 2222');
    expect(flat[3]!.sub).toBe('Kilépett');
  });

  it('is empty before the first answer', () => {
    expect(buildSearchHits(undefined, 'hu', labels)).toEqual({ flat: [], groups: [] });
  });
});

describe('lead sources report', () => {
  it('shows channels with their win rate, and tagged campaigns and pages', async () => {
    overrides.set('/reports/lead-sources', () => ({
      period: { from: '2026-01-01', to: '2026-03-31' },
      total: 8,
      won: 3,
      by_channel: [
        { channel: 'paid', leads: 5, won: 2 },
        { channel: 'organic', leads: 3, won: 1 },
      ],
      by_campaign: [{ channel: 'paid', utm_source: 'google', utm_medium: 'cpc', utm_campaign: 'tavasz', leads: 5, won: 2 }],
      by_page: [{ landing_page: '/hutokamra', leads: 4, won: 2 }],
    }));
    renderPage(<LeadSourcesReport />);
    expect(await screen.findByText('Fizetett hirdetés')).toBeInTheDocument();
    expect(screen.getByText('Keresőből (ingyenes)')).toBeInTheDocument();
    expect(screen.getByText('8 érdeklődés, ebből 3 megnyert.')).toBeInTheDocument();
    // 2 of 5, in the channel table and again in the campaign table.
    expect(screen.getAllByText('40%')).toHaveLength(2);
    expect(screen.getByText('tavasz')).toBeInTheDocument();
    expect(screen.getByText('google / cpc')).toBeInTheDocument();
    expect(screen.getByText('/hutokamra')).toBeInTheDocument();
  });

  it('says so when no website leads came in', async () => {
    overrides.set('/reports/lead-sources', () => ({
      period: { from: '2026-01-01', to: '2026-03-31' },
      total: 0,
      won: 0,
      by_channel: [],
      by_campaign: [],
      by_page: [],
    }));
    renderPage(<LeadSourcesReport />);
    expect(await screen.findByText(/nem érkezett weboldalas érdeklődés/)).toBeInTheDocument();
  });

  it('rounds the win rate and survives zero leads', () => {
    expect(winRate(1, 3)).toBe('33%');
    expect(winRate(0, 0)).toBe('—');
  });
});
