// The photo-list editor in settings: one list per vehicle kind and per walkaround, with
// the general list as the fallback a vehicle kind without its own list is served.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { renderPage } from './harness';
import { overrides, resetOverrides, resetWrites, writes } from './client-mock';
import { ZoneTemplates } from '@/components/inspections/ZoneTemplates';

vi.mock('@/lib/api/client', () => import('./client-mock'));

const COOLING = 1; // fixtures: coolingProjectType
const HEATING = 2;

function zone(
  projectTypeId: number | null,
  kind: string,
  key: string,
  position: number,
  extra: Record<string, unknown> = {},
) {
  return {
    id: position,
    set_key: `${projectTypeId === null ? 'default' : 'type'}:${kind}`,
    project_type_id: projectTypeId,
    kind,
    zone_key: key,
    position,
    title: `Cím ${key}`,
    instruction: `Fotó: ${key}`,
    optional: false,
    required: true,
    ...extra,
  };
}

/** What the server answers: the general list, except the cooling type which has its own outgo list. */
function serve() {
  overrides.set('/inspections/templates', (search) => {
    const kind = String(search?.kind);
    if (Number(search?.project_type_id) === COOLING && kind === 'checkin') {
      return {
        items: [zone(COOLING, kind, 'type_plate', 1), zone(COOLING, kind, 'rear', 2, { optional: true })],
      };
    }
    return { items: [zone(null, kind, 'front', 1), zone(null, kind, 'rear', 2)] };
  });
}

beforeEach(() => {
  resetWrites();
  serve();
});
afterEach(() => resetOverrides());

const keyInputs = () => screen.getAllByLabelText('Zónakód') as HTMLInputElement[];
const pick = (label: string, value: string) =>
  fireEvent.change(screen.getByLabelText(label), { target: { value } });

describe('ZoneTemplates', () => {
  it('starts on the general intake list, in order, with no note about inheriting', async () => {
    renderPage(<ZoneTemplates />);
    await waitFor(() => expect(keyInputs().map((i) => i.value)).toEqual(['front', 'rear']));
    expect(screen.queryByText(/nincs saját listája/)).toBeNull();
    expect(screen.queryByRole('button', { name: 'Saját lista törlése' })).toBeNull();
  });

  it('says so when a vehicle kind is served the general list, and saving gives it its own', async () => {
    renderPage(<ZoneTemplates />);
    await waitFor(() => expect(keyInputs()).toHaveLength(2));

    pick('Járműtípus', String(HEATING));
    await screen.findByText(/nincs saját listája/);
    expect(screen.queryByRole('button', { name: 'Saját lista törlése' })).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Mentés' }));
    await waitFor(() => expect(writes).toHaveLength(1));
    const [write] = writes;
    expect(write?.method).toBe('PUT');
    expect(write?.body).toMatchObject({
      project_type_id: HEATING,
      kind: 'checkout',
      zones: [
        { zone_key: 'front', position: 1, required: true },
        { zone_key: 'rear', position: 2, required: true },
      ],
    });
  });

  it('keeps the two walkarounds apart and offers to remove a list a vehicle kind owns', async () => {
    renderPage(<ZoneTemplates />);
    await waitFor(() => expect(keyInputs()).toHaveLength(2));

    pick('Járműtípus', String(COOLING));
    pick('Körbejárás', 'checkin');
    await screen.findByText('Saját lista (2 fotó)');
    await waitFor(() => expect(keyInputs().map((i) => i.value)).toEqual(['type_plate', 'rear']));

    // The optional flag of the second zone came through.
    const checkboxes = screen.getAllByRole('checkbox') as HTMLInputElement[];
    expect(checkboxes.map((c) => c.checked)).toEqual([false, true]);

    fireEvent.click(screen.getByRole('button', { name: 'Saját lista törlése' }));
    const dialog = await screen.findByRole('dialog');
    fireEvent.click(within(dialog).getByRole('button', { name: 'Törlés' }));
    await waitFor(() => expect(writes).toHaveLength(1));
    expect(writes[0]).toMatchObject({
      method: 'DELETE',
      search: { project_type_id: COOLING, kind: 'checkin' },
    });
  });

  it('saves the rows in the order they are in after moving one', async () => {
    renderPage(<ZoneTemplates />);
    await waitFor(() => expect(keyInputs()).toHaveLength(2));

    fireEvent.click(screen.getAllByRole('button', { name: 'Lejjebb' })[0] as HTMLElement);
    expect(keyInputs().map((i) => i.value)).toEqual(['rear', 'front']);
    fireEvent.click(screen.getByRole('button', { name: 'Mentés' }));

    await waitFor(() => expect(writes).toHaveLength(1));
    expect(writes[0]?.body).toMatchObject({
      project_type_id: null,
      kind: 'checkout',
      zones: [
        { zone_key: 'rear', position: 1 },
        { zone_key: 'front', position: 2 },
      ],
    });
  });

  it('never sends a zone that is missing its heading, key or instruction', async () => {
    renderPage(<ZoneTemplates />);
    await waitFor(() => expect(keyInputs()).toHaveLength(2));

    fireEvent.click(screen.getByRole('button', { name: 'Zóna hozzáadása' }));
    expect(keyInputs()).toHaveLength(3);
    fireEvent.click(screen.getByRole('button', { name: 'Mentés' }));

    await waitFor(() => expect(writes).toHaveLength(1));
    const zones = (writes[0]?.body as { zones: unknown[] }).zones;
    expect(zones).toHaveLength(2);
  });
});
