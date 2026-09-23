'use client';

import {
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type ColumnDef,
  type SortingState,
  type VisibilityState,
} from '@tanstack/react-table';
import { ArrowDown, ArrowUp, ChevronsUpDown } from 'lucide-react';
import { useMemo } from 'react';
import { TableSkeleton } from '@/components/ui/LoadingState';
import { EmptyState } from '@/components/ui/EmptyState';

declare module '@tanstack/react-table' {
  interface ColumnMeta<TData, TValue> {
    /** Server sort key for this column (e.g. 'name', backend `-` prefix for descending). */
    sortKey?: string;
    /** Right-align money and date columns (mono, tabular figures). */
    align?: 'left' | 'right';
  }
}

export interface TableSort {
  key: string;
  dir: 'asc' | 'desc';
}

/** Next sort state when a sortable header is activated: asc → desc → cleared. */
export function nextSort(current: TableSort | null, key: string): TableSort | null {
  if (!current || current.key !== key) return { key, dir: 'asc' };
  if (current.dir === 'asc') return { key, dir: 'desc' };
  return null;
}

/** Menu items for ColumnMenu: explicit id wins, else the accessor key. */
export function columnMenuItems<T>(
  columns: ColumnDef<T, unknown>[],
): { id: string; label: string }[] {
  return columns.flatMap((c) => {
    const id =
      'id' in c && typeof c.id === 'string'
        ? c.id
        : 'accessorKey' in c && typeof c.accessorKey === 'string'
          ? c.accessorKey
          : null;
    if (!id || typeof c.header !== 'string') return [];
    return [{ id, label: c.header }];
  });
}

export function DataTable<T>({
  columns,
  data,
  isLoading,
  isFetching,
  emptyTitle,
  emptyFilteredTitle,
  filtered,
  getRowId,
  density,
  sort,
  onSort,
  columnVisibility,
  onColumnVisibilityChange,
}: {
  columns: ColumnDef<T, unknown>[];
  data: T[];
  /** Initial load (no rows yet). Shows a table-shaped skeleton, not a bare spinner. */
  isLoading?: boolean;
  /** Background refetch (page/filter/sort change with old rows kept via
   *  keepPreviousData). Shows a thin progress bar + dimmed table instead of
   *  swapping content out — previously refetches had zero feedback and felt
   *  like buffering. */
  isFetching?: boolean;
  /** Shown when the list itself is empty. */
  emptyTitle: string;
  /** Shown when filters/search hide everything. Defaults to emptyTitle. */
  emptyFilteredTitle?: string;
  /** Whether any search, filter or non-default state is active. */
  filtered?: boolean;
  getRowId?: (row: T) => string;
  /** From the user's preferences; compact tightens row padding. */
  density?: string;
  /** Server-side sort. Null means the backend default order. */
  sort?: TableSort | null;
  onSort?: (key: string) => void;
  columnVisibility?: VisibilityState;
  onColumnVisibilityChange?: (next: VisibilityState) => void;
}) {
  const sorting: SortingState = useMemo(() => {
    if (!sort) return [];
    const col = columns.find((c) => c.meta?.sortKey === sort.key);
    const id = col && 'accessorKey' in col ? col.accessorKey : undefined;
    if (typeof id !== 'string') return [];
    return [{ id, desc: sort.dir === 'desc' }];
  }, [sort, columns]);

  const table = useReactTable({
    data,
    columns,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    manualSorting: true,
    state: { sorting, columnVisibility: columnVisibility ?? {} },
    onColumnVisibilityChange: (updater) => {
      if (!onColumnVisibilityChange) return;
      const next =
        typeof updater === 'function' ? updater(columnVisibility ?? {}) : updater;
      onColumnVisibilityChange(next);
    },
    getRowId: getRowId ? (row) => getRowId(row) : undefined,
  });

  if (isLoading) return <TableSkeleton rows={8} />;
  if (data.length === 0) {
    return <EmptyState title={filtered ? (emptyFilteredTitle ?? emptyTitle) : emptyTitle} />;
  }

  return (
    <div className="relative">
      {isFetching && (
        <div
          className="absolute inset-x-0 top-0 z-10 h-0.5 overflow-hidden rounded-full bg-steel-200"
          role="progressbar"
          aria-label="loading"
        >
          <div className="h-full w-1/3 animate-pulse bg-steel-900" />
        </div>
      )}
      <div
        className={`table-container${density === 'compact' ? ' density-compact' : ''}${isFetching ? ' opacity-70 transition-opacity' : ''}`}
      >
      <table className="table">
        <thead>
          {table.getHeaderGroups().map((hg) => (
            <tr key={hg.id}>
              {hg.headers.map((h) => {
                const sortKey = h.column.columnDef.meta?.sortKey;
                const active = sort?.key === sortKey;
                const right = h.column.columnDef.meta?.align === 'right';
                return (
                  <th
                    key={h.id}
                    scope="col"
                    // Important: beats the `.table th` text-left rule.
                    className={right ? '!text-right' : undefined}
                    aria-sort={
                      sortKey ? (active ? (sort?.dir === 'asc' ? 'ascending' : 'descending') : 'none') : undefined
                    }
                  >
                    {h.isPlaceholder ? null : sortKey && onSort ? (
                      <button
                        type="button"
                        className="inline-flex items-center gap-1 font-medium hover:text-steel-900"
                        onClick={() => onSort(sortKey)}
                      >
                        {flexRender(h.column.columnDef.header, h.getContext())}
                        {active ? (
                          sort?.dir === 'asc' ? (
                            <ArrowUp className="h-3.5 w-3.5" aria-hidden />
                          ) : (
                            <ArrowDown className="h-3.5 w-3.5" aria-hidden />
                          )
                        ) : (
                          <ChevronsUpDown className="h-3.5 w-3.5 text-steel-500" aria-hidden />
                        )}
                      </button>
                    ) : (
                      flexRender(h.column.columnDef.header, h.getContext())
                    )}
                  </th>
                );
              })}
            </tr>
          ))}
        </thead>
        <tbody>
          {table.getRowModel().rows.map((row) => (
            <tr key={row.id}>
              {row.getVisibleCells().map((cell) => (
                <td
                  key={cell.id}
                  className={cell.column.columnDef.meta?.align === 'right' ? 'text-right' : undefined}
                >
                  {flexRender(cell.column.columnDef.cell, cell.getContext())}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      </div>
    </div>
  );
}
