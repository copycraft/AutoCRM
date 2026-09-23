'use client';

// Walkaround zone templates: the fixed zone order the phone leads the inspector
// through. `default` always applies; `cooling` adds the cargo extras for
// refrigerated bodies. Admin-only (ManageConfiguration server-side).

import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslations } from 'next-intl';
import { inspectionsApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { ErrorState } from '@/components/ui/ErrorState';
import { LoadingState } from '@/components/ui/LoadingState';
import type { ZoneTemplate } from '@/lib/api/types';

interface Row {
  zone_key: string;
  instruction: string;
  optional: boolean;
}

export function ZoneTemplates() {
  const t = useTranslations('settings');
  const def = useQuery({
    queryKey: qk.inspectionTemplates('default'),
    queryFn: () => inspectionsApi.templates(),
  });

  return (
    <section className="card">
      <div className="card-header">
        <h2 className="text-section font-semibold">{t('zonesTitle')}</h2>
      </div>
      <div className="card-content">
        <p className="text-metadata text-steel-500">{t('zonesBody')}</p>
        <div className="mt-3">
          {def.isPending ? (
            <LoadingState />
          ) : def.isError ? (
            <ErrorState error={def.error} onRetry={() => void def.refetch()} />
          ) : (
            <TemplateEditor setKey="default" initial={def.data.items} />
          )}
        </div>
      </div>
    </section>
  );
}

function TemplateEditor({ setKey, initial }: { setKey: string; initial: ZoneTemplate[] }) {
  const t = useTranslations('settings');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const [rows, setRows] = useState<Row[]>(
    [...initial]
      .sort((a, b) => a.position - b.position)
      .map((z) => ({ zone_key: z.zone_key, instruction: z.instruction, optional: z.optional })),
  );

  const save = useMutation({
    mutationFn: () =>
      inspectionsApi.saveTemplates(setKey, {
        zones: rows
          .map((r, i) => ({
            zone_key: r.zone_key.trim(),
            position: i + 1,
            instruction: r.instruction.trim(),
            optional: r.optional,
            required: true,
          }))
          .filter((r) => r.zone_key !== '' && r.instruction !== ''),
      }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: qk.inspectionTemplates(setKey) });
    },
  });

  const set = (i: number, patch: Partial<Row>) =>
    setRows((prev) => prev.map((r, j) => (j === i ? { ...r, ...patch } : r)));

  return (
    <div className="space-y-2">
      {rows.map((row, i) => (
        <div key={i} className="flex flex-wrap items-center gap-2">
          <span className="w-8 font-mono text-metadata text-steel-500">{i + 1}.</span>
          <input
            className="input w-36 font-mono"
            value={row.zone_key}
            onChange={(e) => set(i, { zone_key: e.target.value })}
            aria-label={t('zoneKey')}
          />
          <input
            className="input min-w-52 flex-1"
            value={row.instruction}
            onChange={(e) => set(i, { instruction: e.target.value })}
            aria-label={t('zoneInstruction')}
          />
          <label className="flex items-center gap-1 text-metadata">
            <input
              type="checkbox"
              className="rounded border-steel-200 accent-steel-900"
              checked={row.optional}
              onChange={(e) => set(i, { optional: e.target.checked })}
            />
            {t('zoneOptional')}
          </label>
          <button
            type="button"
            className="btn-ghost btn-sm"
            onClick={() => setRows((prev) => prev.filter((_, j) => j !== i))}
            aria-label={tc('delete')}
          >
            ✕
          </button>
        </div>
      ))}
      <div className="flex flex-wrap gap-2 pt-2">
        <button
          type="button"
          className="btn-secondary btn-sm"
          onClick={() => setRows((prev) => [...prev, { zone_key: '', instruction: '', optional: false }])}
        >
          {t('zoneAdd')}
        </button>
        <button
          type="button"
          className="btn-primary btn-sm"
          disabled={save.isPending}
          onClick={() => save.mutate()}
        >
          {save.isPending ? tc('saving') : tc('save')}
        </button>
        {save.isError && (
          <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">
            {errorMessage(save.error, ter, ter('unknownError'))}
          </p>
        )}
      </div>
    </div>
  );
}
