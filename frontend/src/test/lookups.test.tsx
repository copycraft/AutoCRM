// `GET /config/lookups`: the server owns every client-facing enumeration, and
// the clients render selects, chips and labels from it. These pin down the
// contract both sides rely on: the hook fetches once, labels resolve, and an
// unknown key reads as itself rather than blank.
import { describe, expect, it, vi } from 'vitest';
import { screen, waitFor } from '@testing-library/react';
import { renderPage } from './harness';
import { humanizeKey, lookupLabel, useLookups } from '@/hooks/useLookups';
import { lookups } from './fixtures';

vi.mock('@/lib/api/client', () => import('./client-mock'));

function Probe({ list, value }: { list: 'damage' | 'verdict' | 'fuel'; value: string }) {
  const { data } = useLookups();
  const items =
    list === 'damage' ? data?.damage_types : list === 'verdict' ? data?.verdicts : data?.fuel_levels;
  return <p>{data ? lookupLabel(items, value) : 'loading'}</p>;
}

describe('server-driven lookups', () => {
  it('labels a damage type and a verdict from the server document', async () => {
    renderPage(
      <>
        <Probe list="damage" value="scratch" />
        <Probe list="verdict" value="dismissed" />
      </>,
    );
    await waitFor(() => expect(screen.getByText('Karcolás')).toBeInTheDocument());
    expect(screen.getByText('Nem sérülés')).toBeInTheDocument();
  });

  it('an unknown key reads as itself, spaced out, never blank', async () => {
    renderPage(<Probe list="fuel" value="some_new_mark" />);
    await waitFor(() => expect(screen.getByText('Some new mark')).toBeInTheDocument());
    expect(lookupLabel(undefined, 'x')).toBe('X');
    expect(humanizeKey('  hot_gas ')).toBe('Hot gas');
  });

  it('the document carries every list the clients render', () => {
    for (const key of [
      'damage_types',
      'severities',
      'verdicts',
      'walkaround_kinds',
      'fuel_levels',
      'heating_fuels',
      'defrost_modes',
      'order_relations',
      'task_entity_types',
      'currencies',
      'invoice_payment_methods',
      'annulment_codes',
      'image_categories',
      'email_themes',
      'error_texts',
    ] as const) {
      expect(lookups[key].length, key).toBeGreaterThan(0);
    }
    expect(lookups.image_categories.find((c) => c.key === 'intake')?.immutable).toBe(true);
    expect(lookups.image_categories.filter((c) => c.attachable).map((c) => c.key)).toEqual([
      'production',
    ]);
  });
});
