import { describe, expect, it, vi } from 'vitest';
import { screen, waitFor, fireEvent } from '@testing-library/react';
import { renderPage } from './harness';
import { ToastProvider, useToast } from '@/components/ui/Toasts';
import { Breadcrumbs } from '@/components/ui/Breadcrumbs';
import { CopyButton } from '@/components/ui/CopyButton';
import { nextSort } from '@/components/tables/DataTable';
import { matchActions, type PaletteAction } from '@/components/search/CommandPalette';
import { csvCell, csvFilename, CSV_BOM } from '@/lib/utils/csv';
import { groupByDay, dayKey } from '@/components/ui/DayGroups';
import { budapestIsoPlus } from '@/components/forms/DateQuickPicks';
import { columnMenuItems } from '@/components/tables/DataTable';
import { loadDraft, clearDraft } from '@/hooks/useFormDraft';
import { lastAssignee, rememberLastUsed } from '@/hooks/useLastUsed';
import { ActiveFilterChips } from '@/components/tables/ActiveFilterChips';
import { CollapsibleSection } from '@/components/ui/CollapsibleSection';
import { useRecent } from '@/hooks/useRecent';
import { useSavedViews } from '@/hooks/useSavedViews';

function RecentProbe() {
  const { items, push, clear } = useRecent('autocrm:test-recent');
  return (
    <div>
      <button onClick={() => push('ABC-123')}>push</button>
      <button onClick={clear}>clear</button>
      <ul>
        {items.map((x) => (
          <li key={x}>{x}</li>
        ))}
      </ul>
    </div>
  );
}

function ViewsProbe() {
  const { views, save, remove } = useSavedViews('test-list');
  return (
    <div>
      <button onClick={() => save('mine', { q: 'x' })}>save</button>
      <button onClick={() => remove('mine')}>remove</button>
      <ul>
        {views.map((v) => (
          <li key={v.name}>{v.name}</li>
        ))}
      </ul>
    </div>
  );
}

function ToastProbe() {
  const toast = useToast();
  return <button onClick={() => toast.success('Kész!')}>fire</button>;
}

describe('QoL primitives', () => {
  it('nextSort cycles asc -> desc -> cleared', () => {
    expect(nextSort(null, 'number')).toEqual({ key: 'number', dir: 'asc' });
    expect(nextSort({ key: 'number', dir: 'asc' }, 'number')).toEqual({
      key: 'number',
      dir: 'desc',
    });
    expect(nextSort({ key: 'number', dir: 'desc' }, 'number')).toBeNull();
    expect(nextSort({ key: 'number', dir: 'asc' }, 'total')).toEqual({
      key: 'total',
      dir: 'asc',
    });
  });

  it('recent store dedups and clears', async () => {
    window.localStorage.clear();
    renderPage(<RecentProbe />);
    fireEvent.click(screen.getByText('push'));
    fireEvent.click(screen.getByText('push'));
    await waitFor(() => {
      expect(screen.getAllByText('ABC-123').length).toBe(1);
    });
    fireEvent.click(screen.getByText('clear'));
    await waitFor(() => {
      expect(screen.queryByText('ABC-123')).toBeNull();
    });
  });

  it('saved views save and remove', async () => {
    window.localStorage.clear();
    renderPage(<ViewsProbe />);
    fireEvent.click(screen.getByText('save'));
    await waitFor(() => {
      expect(screen.getByText('mine')).toBeTruthy();
    });
    fireEvent.click(screen.getByText('remove'));
    await waitFor(() => {
      expect(screen.queryByText('mine')).toBeNull();
    });
  });

  it('toast provider shows success toasts', async () => {
    renderPage(
      <ToastProvider>
        <ToastProbe />
      </ToastProvider>,
    );
    fireEvent.click(screen.getByText('fire'));
    await waitFor(() => {
      expect(screen.getByText('Kész!')).toBeTruthy();
    });
  });

  it('breadcrumbs render links and current page', () => {
    renderPage(
      <Breadcrumbs items={[{ href: '/hu/orders', label: 'Megrendelések' }, { label: '#1' }]} />,
    );
    expect(screen.getByText('Megrendelések')).toBeTruthy();
    expect(screen.getByText('#1')).toBeTruthy();
  });

  it('copy button is null for empty values and copies otherwise', async () => {    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    const { container } = renderPage(<CopyButton value="" label="Rendszám" />);
    expect(container.textContent).toBe('');
    renderPage(
      <ToastProvider>
        <CopyButton value="ABC-123" label="Rendszám" />
      </ToastProvider>,
    );
    fireEvent.click(screen.getByLabelText('Rendszám másolása'));
    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith('ABC-123');
    });
  });

  it('palette matchActions returns all on empty and filters by label/hint/id', () => {
    const actions = [
      { id: 'orders', href: '/hu/orders', label: 'Megrendelések', hint: 'g o', icon: undefined as unknown as PaletteAction['icon'] },
      { id: 'new-order', href: '/hu/orders/new', label: 'Új megrendelés', hint: 'n', icon: undefined as unknown as PaletteAction['icon'] },
    ];
    expect(matchActions('', actions).length).toBe(2);
    expect(matchActions('megrend', actions).length).toBe(2);
    expect(matchActions('új', actions).map((a) => a.id)).toEqual(['new-order']);
    expect(matchActions('g o', actions).map((a) => a.id)).toEqual(['orders']);
    expect(matchActions('zzz', actions).length).toBe(0);
  });

  it('csvCell quotes per RFC 4180 and filenames carry the Budapest date', () => {
    expect(csvCell(null)).toBe('""');
    expect(csvCell('a;b')).toBe('"a;b"');
    expect(csvCell('mondd "igen"')).toBe('"mondd ""igen"""');
    expect(CSV_BOM.charCodeAt(0)).toBe(0xfeff);
    expect(csvFilename('leadek')).toMatch(/^leadek-\d{4}-\d{2}-\d{2}\.csv$/);
  });

  it('groupByDay keeps order and splits on Budapest calendar days', () => {
    const items = [
      { at: '2026-09-13T10:00:00+02:00' },
      { at: '2026-09-13T09:00:00+02:00' },
      { at: '2026-09-12T23:00:00+02:00' },
    ];
    const groups = groupByDay(items, (x) => x.at);
    expect(groups.length).toBe(2);
    expect(groups[0]?.items.length).toBe(2);
    expect(groups[1]?.items.length).toBe(1);
    expect(dayKey('2026-09-13T10:00:00+02:00')).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it('budapestIsoPlus lands on the right calendar days', () => {
    const today = budapestIsoPlus(0);
    const plus7 = budapestIsoPlus(7);
    expect(today).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    const diff = Date.parse(`${plus7}T12:00:00Z`) - Date.parse(`${today}T12:00:00Z`);
    expect(diff).toBe(7 * 86_400_000);
  });

  it('budapestIsoPlus follows the Budapest calendar across the UTC midnight boundary', () => {
    // 2026-09-13T22:30:00Z is already 2026-09-14 in Budapest (CEST, +02:00) while
    // still 2026-09-13 in UTC. The old todayIso() reported the UTC day here, so the
    // dashboard and the lead detail disagreed about expiring quotes every night.
    vi.useFakeTimers();
    try {
      vi.setSystemTime(new Date('2026-09-13T22:30:00Z'));
      expect(new Date().toISOString().slice(0, 10)).toBe('2026-09-13');
      expect(budapestIsoPlus(0)).toBe('2026-09-14');
      expect(budapestIsoPlus(7)).toBe('2026-09-21');
    } finally {
      vi.useRealTimers();
    }
  });

  it('columnMenuItems prefers explicit ids and skips non-string headers', () => {
    const cols = columnMenuItems([
      { accessorKey: 'title', header: 'Cím' },
      { accessorKey: 'created_at', id: 'age', header: 'Kor' },
      { accessorKey: 'x', header: 42 },
    ] as never);
    expect(cols).toEqual([
      { id: 'title', label: 'Cím' },
      { id: 'age', label: 'Kor' },
    ]);
  });

  it('drafts round-trip through localStorage', () => {
    window.localStorage.clear();
    expect(loadDraft('probe')).toBeNull();
    window.localStorage.setItem('autocrm:draft:probe', JSON.stringify({ title: 'Váz' }));
    expect(loadDraft<{ title: string }>('probe')).toEqual({ title: 'Váz' });
    clearDraft('probe');
    expect(loadDraft('probe')).toBeNull();
  });

  it('lastAssignee parses me, ids and garbage', () => {
    window.localStorage.clear();
    expect(lastAssignee()).toBeNull();
    rememberLastUsed('assignee', 'me');
    expect(lastAssignee()).toBe('me');
    rememberLastUsed('assignee', '42');
    expect(lastAssignee()).toBe(42);
    rememberLastUsed('assignee', 'zzz');
    expect(lastAssignee()).toBeNull();
  });

  it('filter chips render and remove one filter at a time', () => {
    const onRemove = vi.fn();
    renderPage(
      <ActiveFilterChips
        chips={[
          { key: 'q', label: 'Keresés: x', onRemove },
          { key: 'open', label: 'Csak nyitottak', onRemove: () => undefined },
        ]}
      />,
    );
    expect(screen.getByText('Keresés: x')).toBeTruthy();
    fireEvent.click(screen.getByLabelText('Keresés: x szűrő törlése'));
    expect(onRemove).toHaveBeenCalledTimes(1);
  });

  it('collapsible sections toggle and persist', async () => {
    window.localStorage.clear();
    renderPage(
      <CollapsibleSection storageKey="test-sec" title="Szakasz">
        <p>Beltartalom</p>
      </CollapsibleSection>,
    );
    expect(screen.getByText('Beltartalom')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { expanded: true }));
    await waitFor(() => {
      expect(screen.queryByText('Beltartalom')).toBeNull();
    });
    expect(window.localStorage.getItem('autocrm:section:test-sec')).toBe('0');
  });

  it('toasts render an undo action that fires and dismisses', async () => {
    const onUndo = vi.fn();
    function UndoProbe() {
      const toast = useToast();
      return (
        <button
          onClick={() => toast.success('Törölve', undefined, { label: 'Visszavonás', onClick: onUndo })}
        >
          fire-undo
        </button>
      );
    }
    renderPage(
      <ToastProvider>
        <UndoProbe />
      </ToastProvider>,
    );
    fireEvent.click(screen.getByText('fire-undo'));
    await waitFor(() => {
      expect(screen.getByText('Visszavonás')).toBeTruthy();
    });
    fireEvent.click(screen.getByText('Visszavonás'));
    expect(onUndo).toHaveBeenCalledTimes(1);
  });
});
