'use client';

// The photo lists the phone walks: one list per vehicle kind (project type) and per
// walkaround — átvétel, the vehicle arriving, and kiadás, the vehicle leaving. A vehicle
// kind without a list of its own is served the general one. Whole lists are replaced in
// order. Admin-only (ManageConfiguration server-side).

import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslations } from 'next-intl';
import { configApi, inspectionsApi, type ZoneKind } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { ConfirmDialog } from '@/components/ui/ConfirmDialog';
import { ErrorState } from '@/components/ui/ErrorState';
import { LoadingState } from '@/components/ui/LoadingState';
import type { ZoneTemplate } from '@/lib/api/types';

interface Row {
  zone_key: string;
  title: string;
  instruction: string;
  optional: boolean;
}

export function ZoneTemplates() {
  const t = useTranslations('settings');
  // null is the general list.
  const [projectTypeId, setProjectTypeId] = useState<number | null>(null);
  const [kind, setKind] = useState<ZoneKind>('checkout');

  const types = useQuery({ queryKey: qk.projectTypes, queryFn: () => configApi.projectTypes() });
  const list = useQuery({
    queryKey: qk.inspectionTemplates(projectTypeId, kind),
    queryFn: () => inspectionsApi.templates(projectTypeId, kind),
  });

  return (
    <section className="card">
      <div className="card-header">
        <h2 className="text-section font-semibold">{t('zonesTitle')}</h2>
      </div>
      <div className="card-content">
        <p className="text-metadata text-steel-500">{t('zonesBody')}</p>

        <div className="mt-3 flex flex-wrap items-end gap-3">
          <div>
            <label className="label" htmlFor="zones-vehicle">{t('zonesVehicle')}</label>
            <select
              id="zones-vehicle"
              className="input"
              value={projectTypeId ?? ''}
              onChange={(e) => setProjectTypeId(e.target.value === '' ? null : Number(e.target.value))}
            >
              <option value="">{t('zonesGeneral')}</option>
              {(types.data?.items ?? [])
                .filter((p) => p.is_active)
                .map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.label_hu}
                  </option>
                ))}
            </select>
          </div>
          <div>
            <label className="label" htmlFor="zones-kind">{t('zonesWalkaround')}</label>
            <select
              id="zones-kind"
              className="input"
              value={kind}
              onChange={(e) => setKind(e.target.value as ZoneKind)}
            >
              <option value="checkout">{t('zonesCheckout')}</option>
              <option value="checkin">{t('zonesCheckin')}</option>
            </select>
          </div>
        </div>

        <div className="mt-3">
          {list.isPending ? (
            <LoadingState />
          ) : list.isError ? (
            <ErrorState error={list.error} onRetry={() => void list.refetch()} />
          ) : (
            // Remounted when the list changes identity (another vehicle or walkaround, or
            // it gains or loses a list of its own), so edits never carry over; a plain save
            // keeps the editor, and its "saved" note, as it is.
            <TemplateEditor
              key={`${projectTypeId ?? 'general'}:${kind}:${list.data.items.some((z) => z.project_type_id !== null)}`}
              projectTypeId={projectTypeId}
              kind={kind}
              initial={list.data.items}
            />
          )}
        </div>
      </div>
    </section>
  );
}

function TemplateEditor({
  projectTypeId,
  kind,
  initial,
}: {
  projectTypeId: number | null;
  kind: ZoneKind;
  initial: ZoneTemplate[];
}) {
  const t = useTranslations('settings');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const [rows, setRows] = useState<Row[]>(
    [...initial]
      .sort((a, b) => a.position - b.position)
      .map((z) => ({
        zone_key: z.zone_key,
        title: z.title,
        instruction: z.instruction,
        optional: z.optional,
      })),
  );
  const [removing, setRemoving] = useState(false);
  const [saved, setSaved] = useState(false);

  // A vehicle kind without its own list is served the general one (items say so).
  const inherited = projectTypeId !== null && initial.every((z) => z.project_type_id === null);
  const owns = projectTypeId !== null && !inherited;

  const refresh = () => qc.invalidateQueries({ queryKey: ['inspection-templates'] });

  const save = useMutation({
    mutationFn: () =>
      inspectionsApi.saveTemplates({
        project_type_id: projectTypeId,
        kind,
        zones: rows
          .map((r) => ({
            zone_key: r.zone_key.trim(),
            title: r.title.trim(),
            instruction: r.instruction.trim(),
            optional: r.optional,
          }))
          .filter((r) => r.zone_key !== '' && r.title !== '' && r.instruction !== '')
          .map((r, i) => ({ ...r, position: i + 1, required: !r.optional })),
      }),
    onSuccess: () => {
      setSaved(true);
      void refresh();
    },
  });

  const remove = useMutation({
    mutationFn: () => inspectionsApi.deleteTemplates(projectTypeId as number, kind),
    onSuccess: () => {
      setRemoving(false);
      void refresh();
    },
  });

  const set = (i: number, patch: Partial<Row>) => {
    setSaved(false);
    setRows((prev) => prev.map((r, j) => (j === i ? { ...r, ...patch } : r)));
  };
  const move = (i: number, by: -1 | 1) => {
    setSaved(false);
    setRows((prev) => {
      const j = i + by;
      if (j < 0 || j >= prev.length) return prev;
      const a = prev[i];
      const b = prev[j];
      if (!a || !b) return prev;
      const next = [...prev];
      next[i] = b;
      next[j] = a;
      return next;
    });
  };

  return (
    <div className="space-y-2">
      {inherited && (
        <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900">
          {t('zonesInherited')}
        </p>
      )}
      {owns && (
        <div className="flex flex-wrap items-center gap-3">
          <p className="text-metadata text-steel-500">{t('zonesOwn', { count: initial.length })}</p>
          <button type="button" className="btn-ghost btn-sm" onClick={() => setRemoving(true)}>
            {t('zonesRemoveOwn')}
          </button>
        </div>
      )}

      {rows.map((row, i) => (
        <div key={i} className="flex flex-wrap items-center gap-2">
          <span className="w-8 font-mono text-metadata text-steel-500">{i + 1}.</span>
          <input
            className="input w-40"
            value={row.title}
            onChange={(e) => set(i, { title: e.target.value })}
            aria-label={t('zoneTitleLabel')}
          />
          <input
            className="input w-44 font-mono"
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
            disabled={i === 0}
            onClick={() => move(i, -1)}
            aria-label={t('zoneUp')}
          >
            ↑
          </button>
          <button
            type="button"
            className="btn-ghost btn-sm"
            disabled={i === rows.length - 1}
            onClick={() => move(i, 1)}
            aria-label={t('zoneDown')}
          >
            ↓
          </button>
          <button
            type="button"
            className="btn-ghost btn-sm"
            onClick={() => {
              setSaved(false);
              setRows((prev) => prev.filter((_, j) => j !== i));
            }}
            aria-label={tc('delete')}
          >
            ✕
          </button>
        </div>
      ))}
      <div className="flex flex-wrap items-center gap-2 pt-2">
        <button
          type="button"
          className="btn-secondary btn-sm"
          onClick={() => {
            setSaved(false);
            setRows((prev) => [...prev, { zone_key: '', title: '', instruction: '', optional: false }]);
          }}
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
        {saved && !save.isError && <span className="text-metadata text-steel-500">{t('zonesSaved')}</span>}
        {save.isError && (
          <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">
            {errorMessage(save.error, ter, ter('unknownError'))}
          </p>
        )}
      </div>

      <ConfirmDialog
        open={removing}
        title={t('zonesRemoveTitle')}
        body={t('zonesRemoveBody')}
        confirmLabel={tc('delete')}
        busy={remove.isPending}
        onConfirm={() => remove.mutate()}
        onClose={() => setRemoving(false)}
      />
      {remove.isError && (
        <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">
          {errorMessage(remove.error, ter, ter('unknownError'))}
        </p>
      )}
    </div>
  );
}
