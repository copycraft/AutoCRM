// Logic audit (inspection journey): the web check-in review must follow the same
// verdict rules as the API and the phone.
//  - INSP-15: a signed inspection is locked; every mutation answers 422 `locked`,
//    so the web must not offer verdict buttons on a signed check-in.
//  - INSP-31/33: a verdict points at a check-out damage only when it says the damage
//    was pre-existing; "new" and "dismissed" stand alone (the phone sends null).
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { NextIntlClientProvider } from 'next-intl';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import messages from '@/messages/hu.json';

const api = vi.hoisted(() => ({
  list: vi.fn(),
  get: vi.fn(),
  comparison: vi.fn(),
  verdict: vi.fn(),
  note: vi.fn(),
}));

vi.mock('@/lib/api/endpoints', () => ({ inspectionsApi: api }));
vi.mock('@/lib/auth/context', async (orig) => ({
  ...(await orig<typeof import('@/lib/auth/context')>()),
  useAuth: () => ({
    user: {
      id: 1,
      email: 'iroda@autotherm.hu',
      display_name: 'Iroda Ilona',
      role: 'office',
      must_change_password: false,
      session_kind: 'web',
    },
  }),
}));

import { InspectionSection } from '@/components/inspections/InspectionSection';

function inspection(id: number, kind: string, status: string) {
  return {
    id,
    order_id: 7,
    kind,
    status,
    vehicle_plate: 'ABC-123',
    vehicle_vin: null,
    inspector_name: 'Szerelő Sanyi',
    driver_name: null,
    location: null,
    odometer: null,
    fuel_level: null,
    battery_pct: null,
    warning_lights: null,
    checkout_id: kind === 'checkin' ? 1 : null,
    customer_comment: null,
    signed_at: status === 'signed' ? '2026-09-10T10:00:00Z' : null,
    created_by: 1,
    created_at: '2026-09-10T09:00:00Z',
    updated_at: '2026-09-10T09:00:00Z',
  };
}

function damage(id: number, inspectionId: number) {
  return {
    id,
    inspection_id: inspectionId,
    zone_key: 'front',
    damage_type: 'scratch',
    severity: 'minor',
    note: null,
    x: null,
    y: null,
    view: 'top',
    created_at: '2026-09-10T09:00:00Z',
  };
}

function detail(insp: ReturnType<typeof inspection>, damages: ReturnType<typeof damage>[]) {
  return { inspection: insp, photos: [], damages, verdicts: [], signatures: [], notes: [] };
}

function setup(checkinStatus: string) {
  const checkin = inspection(2, 'checkin', checkinStatus);
  const checkout = inspection(1, 'checkout', 'signed');
  api.list.mockResolvedValue({ items: [checkin] });
  api.get.mockResolvedValue(detail(checkin, [damage(20, 2)]));
  api.comparison.mockResolvedValue({
    checkin: detail(checkin, [damage(20, 2)]),
    checkout: detail(checkout, [damage(5, 1)]),
    suggestions: [{ checkin_damage_id: 20, checkout_damage_id: 5, suggested: 'preexisting' }],
  });
  api.verdict.mockResolvedValue({});
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <NextIntlClientProvider locale="hu" messages={messages} timeZone="Europe/Budapest">
      <QueryClientProvider client={client}>
        <InspectionSection orderId={7} />
      </QueryClientProvider>
    </NextIntlClientProvider>,
  );
}

async function openCard() {
  const toggle = await screen.findByRole('button', { expanded: false });
  fireEvent.click(toggle);
  await waitFor(() => expect(api.comparison).toHaveBeenCalled());
  await screen.findByText(messages.orders.verdictMatches, { exact: false });
}

describe('check-in verdicts on the web', () => {
  beforeEach(() => vi.clearAllMocks());

  it('a signed check-in offers no verdict buttons', async () => {
    setup('signed');
    await openCard();
    expect(screen.queryByRole('button', { name: messages.orders.verdictDismissed })).toBeNull();
  });

  it('a draft check-in offers verdict buttons', async () => {
    setup('draft');
    await openCard();
    expect(
      await screen.findByRole('button', { name: messages.orders.verdictDismissed }),
    ).toBeTruthy();
  });

  it('a "new" verdict does not link the suggested check-out damage', async () => {
    setup('draft');
    await openCard();
    fireEvent.click(await screen.findByRole('button', { name: messages.orders.verdictNew }));
    await waitFor(() => expect(api.verdict).toHaveBeenCalled());
    expect(api.verdict).toHaveBeenCalledWith(2, {
      checkin_damage_id: 20,
      checkout_damage_id: null,
      verdict: 'new',
    });
  });

  it('a "pre-existing" verdict links the suggested check-out damage', async () => {
    setup('draft');
    await openCard();
    fireEvent.click(
      await screen.findByRole('button', { name: messages.orders.verdictPreexisting }),
    );
    await waitFor(() => expect(api.verdict).toHaveBeenCalled());
    expect(api.verdict).toHaveBeenCalledWith(2, {
      checkin_damage_id: 20,
      checkout_damage_id: 5,
      verdict: 'preexisting',
    });
  });
});
