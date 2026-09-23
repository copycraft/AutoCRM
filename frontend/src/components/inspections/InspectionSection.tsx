'use client';

// Handover inspections (átadás-átvétel) inside the Átvételi lap: the rental-company
// style check-out / check-in damage record. Created on the phone (guided
// walkaround) — the web reads history, reviews check-in verdicts and annotates
// locked inspections. There is deliberately no create button here.

import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslations } from 'next-intl';
import { inspectionsApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { canChangeStage, useAuth } from '@/lib/auth/context';
import { ErrorState } from '@/components/ui/ErrorState';
import { LoadingState } from '@/components/ui/LoadingState';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { DateDisplay } from '@/components/ui/DateDisplay';
import type {
  Inspection,
  InspectionComparison,
  InspectionDamage,
} from '@/lib/api/types';

const DAMAGE_HU: Record<string, string> = {
  scratch: 'Karcolás',
  dent: 'Horpadás',
  crack: 'Repedés',
  chip: 'Lepattanás',
  broken: 'Törött alkatrész',
  missing: 'Hiányzó alkatrész',
  stain: 'Folt',
  tear: 'Szakadás',
  other: 'Egyéb',
};

const SEVERITY_HU: Record<string, string> = {
  minor: 'Enyhe',
  moderate: 'Közepes',
  severe: 'Súlyos',
};

function kindLabel(kind: string, t: (k: string) => string): string {
  return kind === 'checkin' ? t('checkin') : t('checkout');
}

export function InspectionSection({ orderId }: { orderId: number }) {
  const t = useTranslations('orders');
  const query = useQuery({
    queryKey: qk.inspections({ order: orderId }),
    queryFn: () => inspectionsApi.list({ order_id: orderId }),
  });

  return (
    <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
      <div className="flex items-center justify-between">
        <h2 className="text-section font-semibold">{t('inspectionTitle')}</h2>
        {query.data && query.data.items.length > 0 && (
          <StatusBadge tone="steel">
            {t('inspectionCount', { count: query.data.items.length })}
          </StatusBadge>
        )}
      </div>
      <p className="mt-1 text-metadata text-steel-500">{t('inspectionPhoneOnly')}</p>
      <div className="mt-3">
        {query.isPending ? (
          <LoadingState />
        ) : query.isError ? (
          <ErrorState error={query.error} onRetry={() => void query.refetch()} />
        ) : query.data.items.length === 0 ? (
          <p className="text-body text-steel-500">{t('inspectionEmpty')}</p>
        ) : (
          <ul className="space-y-3">
            {query.data.items.map((inspection) => (
              <InspectionCard key={inspection.id} inspection={inspection} />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}

function InspectionCard({ inspection }: { inspection: Inspection }) {
  const t = useTranslations('orders');
  const [open, setOpen] = useState(false);
  const detail = useQuery({
    queryKey: qk.inspection(inspection.id),
    queryFn: () => inspectionsApi.get(inspection.id),
    enabled: open,
  });

  return (
    <li className="card">
      <button
        type="button"
        className="flex w-full flex-wrap items-center gap-x-4 gap-y-1 p-4 text-left"
        onClick={() => setOpen((o) => !o)}
        aria-expanded={open}
      >
        <span className="min-w-0 flex-1">
          <span className="block text-body font-medium">
            {kindLabel(inspection.kind, t)} · {inspection.vehicle_plate}
          </span>
          <span className="block text-metadata text-steel-500">
            {inspection.inspector_name} ·{' '}
            <DateDisplay withTime value={inspection.signed_at ?? inspection.created_at} />
          </span>
        </span>
        <StatusBadge tone={inspection.status === 'signed' ? 'done' : 'steel'}>
          {inspection.status === 'signed' ? t('inspectionSigned') : t('inspectionDraft')}
        </StatusBadge>
        <span className="text-metadata text-steel-500" aria-hidden>
          {open ? '▾' : '▸'}
        </span>
      </button>
      {open && (
        <div className="border-t border-steel-200 p-4">
          {detail.isPending ? (
            <LoadingState />
          ) : detail.isError || !detail.data ? (
            <ErrorState error={detail.error} onRetry={() => void detail.refetch()} />
          ) : (
            <InspectionDetailView
              inspectionId={inspection.id}
              kind={inspection.kind}
              detail={detail.data}
            />
          )}
        </div>
      )}
    </li>
  );
}

function InspectionDetailView({
  inspectionId,
  kind,
  detail,
}: {
  inspectionId: number;
  kind: string;
  detail: import('@/lib/api/types').InspectionDetail;
}) {
  const t = useTranslations('orders');
  const { inspection, photos, damages, verdicts, signatures, notes } = detail;
  const overviews = photos.filter((p) => p.purpose === 'overview');

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <Info label={t('inspectionInspector')} value={inspection.inspector_name} />
        <Info label={t('inspectionDriver')} value={inspection.driver_name} />
        <Info label={t('inspectionLocation')} value={inspection.location} />
        <Info
          label={t('intakeMileage')}
          value={inspection.odometer != null ? `${inspection.odometer.toLocaleString('hu-HU')} km` : null}
          mono
        />
        <Info label={t('intakeFuel')} value={inspection.fuel_level} mono />
        <Info
          label={t('inspectionBattery')}
          value={inspection.battery_pct != null ? `${inspection.battery_pct} %` : null}
          mono
        />
        <Info label={t('inspectionWarnings')} value={inspection.warning_lights} />
      </div>

      {overviews.length > 0 && (
        <div>
          <h3 className="text-body font-semibold">{t('inspectionZones')}</h3>
          <ul className="mt-2 grid grid-cols-2 gap-2 md:grid-cols-4">
            {overviews.map((photo) => (
              <li key={photo.id}>
                <figure>
                  {photo.display_url ?? photo.thumb_url ? (
                    // Plain img: presigned S3 URLs never match next/image remotePatterns.
                    <img
                      src={photo.display_url ?? photo.thumb_url ?? ''}
                      alt={photo.zone_key}
                      className="aspect-[4/3] w-full rounded-lg border border-steel-200 object-cover"
                      loading="lazy"
                    />
                  ) : (
                    <div className="aspect-[4/3] w-full rounded-lg bg-panel" aria-hidden />
                  )}
                  <figcaption className="mt-1 font-mono text-metadata text-steel-500">
                    {photo.zone_key}
                  </figcaption>
                </figure>
              </li>
            ))}
          </ul>
        </div>
      )}

      {damages.length > 0 && (
        <div>
          <h3 className="text-body font-semibold">
            {t('inspectionDamages')} ({damages.length})
          </h3>
          <ul className="mt-2 space-y-2">
            {damages.map((damage) => (
              <DamageRow key={damage.id} damage={damage} />
            ))}
          </ul>
        </div>
      )}

      {signatures.length > 0 && (
        <p className="text-metadata text-steel-500">
          {t('inspectionSignatures')}:{' '}
          {signatures.map((s) => `${s.name} (${s.role})`).join(' · ')}
        </p>
      )}

      {kind === 'checkin' && <ComparisonView inspectionId={inspectionId} />}

      <NotesList notes={notes} />
      <NoteComposer inspectionId={inspectionId} />
    </div>
  );
}

function DamageRow({ damage }: { damage: InspectionDamage }) {
  return (
    <li className="rounded-lg border border-steel-200 p-3">
      <p className="flex flex-wrap items-center gap-2 text-body font-medium">
        <span className="font-mono text-metadata text-steel-500">{damage.zone_key}</span>
        {DAMAGE_HU[damage.damage_type] ?? damage.damage_type}
        <StatusBadge tone={damage.severity === 'severe' ? 'signal' : 'steel'}>
          {SEVERITY_HU[damage.severity] ?? damage.severity}
        </StatusBadge>
      </p>
      {damage.note && <p className="mt-1 text-body">{damage.note}</p>}
    </li>
  );
}

function ComparisonView({ inspectionId }: { inspectionId: number }) {
  const t = useTranslations('orders');
  const ter = useTranslations('errors');
  const { user } = useAuth();
  const qc = useQueryClient();
  const query = useQuery({
    queryKey: qk.inspectionComparison(inspectionId),
    queryFn: () => inspectionsApi.comparison(inspectionId),
  });
  const review = canChangeStage(user);

  const verdict = useMutation({
    mutationFn: (body: { checkin_damage_id: number; checkout_damage_id: number | null; verdict: string }) =>
      inspectionsApi.verdict(inspectionId, body),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: qk.inspectionComparison(inspectionId) });
      void qc.invalidateQueries({ queryKey: qk.inspection(inspectionId) });
    },
  });

  if (query.isPending) return <LoadingState />;
  if (query.isError || !query.data) {
    return <ErrorState error={query.error} onRetry={() => void query.refetch()} />;
  }
  const comparison: InspectionComparison = query.data;
  const verdictByDamage = new Map(comparison.checkin.verdicts.map((v) => [v.checkin_damage_id, v]));

  return (
    <div>
      <h3 className="text-body font-semibold">{t('inspectionComparison')}</h3>
      {comparison.checkin.damages.length === 0 ? (
        <p className="mt-1 text-body text-steel-500">{t('inspectionNoNewDamage')}</p>
      ) : (
        <ul className="mt-2 space-y-2">
          {comparison.checkin.damages.map((damage) => {
            const suggestion = comparison.suggestions.find(
              (s) => s.checkin_damage_id === damage.id,
            );
            const current = verdictByDamage.get(damage.id);
            const matched = suggestion?.checkout_damage_id
              ? comparison.checkout.damages.find((d) => d.id === suggestion.checkout_damage_id)
              : null;
            return (
              <li key={damage.id} className="rounded-lg border border-steel-200 p-3">
                <p className="flex flex-wrap items-center gap-2 text-body font-medium">
                  <span className="font-mono text-metadata text-steel-500">{damage.zone_key}</span>
                  {DAMAGE_HU[damage.damage_type] ?? damage.damage_type}
                  {suggestion?.suggested === 'preexisting' ? (
                    <StatusBadge tone="cold">{t('verdictPreexisting')}</StatusBadge>
                  ) : (
                    <StatusBadge tone="signal">{t('verdictNew')}</StatusBadge>
                  )}
                  {current && (
                    <StatusBadge tone="done">
                      {t('verdictDecided', { verdict: current.verdict })}
                    </StatusBadge>
                  )}
                </p>
                {matched && (
                  <p className="mt-1 text-metadata text-steel-500">
                    {t('verdictMatches')}: {matched.zone_key} ·{' '}
                    {DAMAGE_HU[matched.damage_type] ?? matched.damage_type}
                  </p>
                )}
                {damage.note && <p className="mt-1 text-body">{damage.note}</p>}
                {review && (
                  <div className="mt-2 flex flex-wrap gap-2">
                    {(
                      [
                        ['preexisting', t('verdictPreexisting')],
                        ['new', t('verdictNew')],
                        ['dismissed', t('verdictDismissed')],
                      ] as const
                    ).map(([value, label]) => (
                      <button
                        key={value}
                        type="button"
                        className="btn-ghost btn-sm"
                        disabled={verdict.isPending}
                        onClick={() =>
                          verdict.mutate({
                            checkin_damage_id: damage.id,
                            checkout_damage_id: suggestion?.checkout_damage_id ?? null,
                            verdict: value,
                          })
                        }
                      >
                        {label}
                      </button>
                    ))}
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      )}
      {verdict.isError && (
        <p className="mt-2 text-body text-steel-900" role="alert">
          {errorMessage(verdict.error, ter, ter('unknownError'))}
        </p>
      )}
      <OdoDelta comparison={comparison} />
    </div>
  );
}

function OdoDelta({ comparison }: { comparison: InspectionComparison }) {
  const t = useTranslations('orders');
  const outOdo = comparison.checkout.inspection.odometer;
  const inOdo = comparison.checkin.inspection.odometer;
  if (outOdo == null || inOdo == null) return null;
  const delta = inOdo - outOdo;
  return (
    <p className="mt-2 font-mono text-metadata text-steel-500">
      {t('inspectionOdoDelta', { delta: delta.toLocaleString('hu-HU') })}
    </p>
  );
}

function NotesList({ notes }: { notes: import('@/lib/api/types').InspectionNote[] }) {
  const t = useTranslations('orders');
  if (notes.length === 0) return null;
  return (
    <div>
      <h3 className="text-body font-semibold">{t('inspectionNotes')}</h3>
      <ul className="mt-2 space-y-2">
        {notes.map((note) => (
          <li key={note.id} className="border-l-2 border-steel-200 pl-3 text-body">
            <p className="whitespace-pre-wrap">{note.body}</p>
            <p className="text-metadata text-steel-500">
              <DateDisplay withTime value={note.created_at} />
            </p>
          </li>
        ))}
      </ul>
    </div>
  );
}

function NoteComposer({ inspectionId }: { inspectionId: number }) {
  const t = useTranslations('orders');
  const ter = useTranslations('errors');
  const { user } = useAuth();
  const qc = useQueryClient();
  const [body, setBody] = useState('');
  const [open, setOpen] = useState(false);
  const mutation = useMutation({
    mutationFn: () => inspectionsApi.note(inspectionId, { body: body.trim() }),
    onSuccess: () => {
      setBody('');
      setOpen(false);
      void qc.invalidateQueries({ queryKey: qk.inspection(inspectionId) });
    },
  });
  if (!canChangeStage(user)) return null;
  if (!open) {
    return (
      <button type="button" className="btn-ghost btn-sm" onClick={() => setOpen(true)}>
        {t('inspectionAddNote')}
      </button>
    );
  }
  return (
    <div className="space-y-2">
      <label className="label" htmlFor={`insp-note-${inspectionId}`}>
        {t('inspectionNoteLabel')}
      </label>
      <textarea
        id={`insp-note-${inspectionId}`}
        rows={2}
        className="input"
        value={body}
        onChange={(e) => setBody(e.target.value)}
      />
      {mutation.isError && (
        <p className="text-body text-steel-900" role="alert">
          {errorMessage(mutation.error, ter, ter('unknownError'))}
        </p>
      )}
      <div className="flex gap-2">
        <button
          type="button"
          className="btn-primary btn-sm"
          disabled={mutation.isPending || !body.trim()}
          onClick={() => mutation.mutate()}
        >
          {t('inspectionNoteSave')}
        </button>
        <button type="button" className="btn-ghost btn-sm" onClick={() => setOpen(false)}>
          {t('cancel')}
        </button>
      </div>
    </div>
  );
}

function Info({ label, value, mono }: { label: string; value: React.ReactNode; mono?: boolean }) {
  return (
    <div className="flex flex-col gap-0.5">
      <dt className="text-metadata font-medium text-steel-500">{label}</dt>
      <dd className={`text-body ${mono ? 'font-mono' : ''}`}>{value ?? '—'}</dd>
    </div>
  );
}
