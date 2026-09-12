'use client';

import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useTranslations } from 'next-intl';
import { ChevronDown, ChevronRight } from 'lucide-react';
import { rawImportApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { ErrorState } from '@/components/ui/ErrorState';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';

type Entity = 'partner' | 'lead' | 'order';

/** One leaf of the source record: its JSON path and its value as text. */
interface Row {
  path: string;
  value: string;
}

/** Flattens the source JSON to leaves. Objects and arrays become dotted/indexed paths. */
function flatten(value: unknown, prefix: string, out: Row[]): void {
  if (value === null || value === undefined) {
    out.push({ path: prefix, value: '—' });
    return;
  }
  if (Array.isArray(value)) {
    if (value.length === 0) {
      out.push({ path: prefix, value: '[]' });
      return;
    }
    value.forEach((item, i) => flatten(item, `${prefix}[${i}]`, out));
    return;
  }
  if (typeof value === 'object') {
    const entries = Object.entries(value as Record<string, unknown>);
    if (entries.length === 0) {
      out.push({ path: prefix, value: '{}' });
      return;
    }
    for (const [key, v] of entries) {
      flatten(v, prefix ? `${prefix}.${key}` : key, out);
    }
    return;
  }
  out.push({ path: prefix, value: String(value) });
}

/**
 * "MiniCRM eredeti adatok" — the source record behind a migrated row (V1.5).
 *
 * `raw_import` has been populated since the first migration and read by nothing: no API,
 * no screen, no search. Someone opening a migrated 2019 order saw a title, a partner and a
 * date, with no way to find out that the van's plate and the customer's address were in a
 * JSONB column. Read-only and collapsed by default; absent entirely on records created in
 * AutoCRM.
 */
export function RawImportPanel({ entity, id }: { entity: Entity; id: number }) {
  const t = useTranslations('rawImport');
  const tc = useTranslations('common');
  const [open, setOpen] = useState(false);
  const [filter, setFilter] = useState('');
  const search = useDebouncedValue(filter, 200).trim().toLowerCase();

  const query = useQuery({
    queryKey: qk.rawImport(entity, id),
    queryFn: () => rawImportApi[entity](id),
    // Only fetched once opened: it is the whole source record.
    enabled: open,
  });

  // Nothing is known until the panel is opened, so the trigger always renders; a record
  // created in AutoCRM answers with nulls and the body says so.
  const rows: Row[] = [];
  if (query.data?.raw_import) {
    flatten(query.data.raw_import, '', rows);
  }
  const visible = search
    ? rows.filter(
        (r) => r.path.toLowerCase().includes(search) || r.value.toLowerCase().includes(search),
      )
    : rows;

  return (
    <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
      <button
        type="button"
        className="flex items-center gap-2 text-section font-semibold"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
      >
        {open ? (
          <ChevronDown className="h-4 w-4" aria-hidden />
        ) : (
          <ChevronRight className="h-4 w-4" aria-hidden />
        )}
        {t('title')}
      </button>

      {open && (
        <div className="mt-3 space-y-3">
          <p className="text-metadata text-steel-500">{t('explanation')}</p>
          {query.isLoading && <p className="text-metadata text-steel-500">{tc('loading')}</p>}
          {query.isError && (
            <ErrorState error={query.error} onRetry={() => void query.refetch()} />
          )}
          {query.data && !query.data.raw_import && (
            <p className="text-metadata text-steel-500">{t('notMigrated')}</p>
          )}
          {query.data?.raw_import && (
            <>
              <div className="flex flex-wrap items-center gap-3">
                <input
                  type="search"
                  className="input max-w-xs"
                  placeholder={t('searchPlaceholder')}
                  value={filter}
                  onChange={(e) => setFilter(e.target.value)}
                  aria-label={t('searchPlaceholder')}
                />
                <p className="font-mono text-metadata text-steel-500">
                  {t('minicrmId')}: {query.data.minicrm_id ?? '—'} · {t('fieldCount', {
                    shown: visible.length,
                    total: rows.length,
                  })}
                </p>
              </div>
              <div className="table-container max-h-[420px] overflow-y-auto">
                <table className="table">
                  <thead>
                    <tr>
                      <th scope="col">{t('field')}</th>
                      <th scope="col">{t('value')}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {visible.length === 0 && (
                      <tr>
                        <td colSpan={2} className="text-metadata text-steel-500">
                          {tc('noResults')}
                        </td>
                      </tr>
                    )}
                    {visible.map((r) => (
                      <tr key={r.path}>
                        <td className="whitespace-nowrap align-top font-mono text-metadata">
                          {r.path}
                        </td>
                        <td className="break-all align-top">{r.value}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </>
          )}
        </div>
      )}
    </section>
  );
}
