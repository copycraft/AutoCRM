'use client';

// The yard board: every place a vehicle can stand, with the vans on it. Drag a card to
// move a van; the move is recorded with who did it and when, and shows in the job's
// history. A bay with more vans than it holds is flagged, not refused. Refreshes every
// 30 seconds, so it can hang on the workshop TV like the pickup board.

import Link from 'next/link';
import { useState } from 'react';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Archive, ArchiveRestore, LogOut, Plus } from 'lucide-react';
import { yardApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { lookupLabel, useLookups } from '@/hooks/useLookups';
import { useToast } from '@/components/ui/Toasts';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { DetailSkeleton } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import { cn } from '@/lib/utils/format';
import type { YardLocation, YardVehicle } from '@/lib/api/types';

const DRAG_TYPE = 'application/x-autocrm-vehicle';

function VehicleCard({ v, draggable }: { v: YardVehicle; draggable: boolean }) {
  const locale = useLocale();
  const t = useTranslations('yard');
  return (
    <div
      draggable={draggable}
      onDragStart={(e) => {
        e.dataTransfer.setData(DRAG_TYPE, JSON.stringify({ vehicle_id: v.vehicle_id, order_id: v.order_id }));
        e.dataTransfer.effectAllowed = 'move';
      }}
      className={cn('rounded-lg border border-steel-200 bg-surface p-2 shadow-panel', draggable && 'cursor-grab active:cursor-grabbing')}
    >
      <p className="font-mono text-body font-semibold">{v.plate ?? v.vin ?? `#${v.vehicle_id}`}</p>
      <p className="truncate text-metadata text-steel-500">{[v.make, v.model].filter(Boolean).join(' ') || '—'}</p>
      {v.order_id && (
        <p className="truncate text-metadata">
          <Link href={`/${locale}/orders/${v.order_id}`} className="underline" draggable={false}>
            #{v.order_number}
          </Link>{' '}
          {v.partner_name}
        </p>
      )}
      <p className="mt-1 flex flex-wrap items-center gap-1 text-metadata text-steel-500">
        {v.stage_label && <StatusBadge tone="steel">{v.stage_label}</StatusBadge>}
        {v.due_date && (
          <span>
            {t('due')} <DateDisplay value={v.due_date} />
          </span>
        )}
      </p>
    </div>
  );
}

function Column({
  title,
  subtitle,
  capacity,
  vehicles,
  tone,
  canMove,
  onDropVehicle,
}: {
  title: string;
  subtitle?: string;
  capacity?: number | null;
  vehicles: YardVehicle[];
  tone?: 'muted';
  canMove: boolean;
  onDropVehicle: (data: { vehicle_id: number; order_id: number | null }) => void;
}) {
  const t = useTranslations('yard');
  const [over, setOver] = useState(false);
  const full = capacity != null && vehicles.length > capacity;
  return (
    <section
      className={cn(
        'flex min-h-40 flex-col rounded-xl border p-3 transition-colors',
        tone === 'muted' ? 'border-dashed border-steel-200 bg-panel' : 'border-steel-200 bg-panel',
        over && 'border-steel-900 bg-steel-200/40',
        full && 'border-signal',
      )}
      onDragOver={(e) => {
        if (!canMove || !e.dataTransfer.types.includes(DRAG_TYPE)) return;
        e.preventDefault();
        setOver(true);
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(e) => {
        setOver(false);
        const raw = e.dataTransfer.getData(DRAG_TYPE);
        if (!raw) return;
        e.preventDefault();
        onDropVehicle(JSON.parse(raw));
      }}
    >
      <header className="mb-2 flex items-baseline justify-between gap-2">
        <h2 className="text-body font-semibold">{title}</h2>
        <span className={cn('font-mono text-metadata', full ? 'font-semibold text-signal' : 'text-steel-500')}>
          {vehicles.length}
          {capacity != null ? ` / ${capacity}` : ''}
        </span>
      </header>
      {subtitle && <p className="-mt-1 mb-2 text-metadata text-steel-500">{subtitle}</p>}
      {full && <p className="mb-2 text-metadata font-medium text-signal">{t('overCapacity')}</p>}
      <div className="flex flex-1 flex-col gap-2">
        {vehicles.map((v) => (
          <VehicleCard key={v.vehicle_id} v={v} draggable={canMove} />
        ))}
      </div>
    </section>
  );
}

export function YardBoard({ canMove, canConfigure }: { canMove: boolean; canConfigure: boolean }) {
  const t = useTranslations('yard');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const { toast } = useToast();
  const { data: lookups } = useLookups();
  const [editing, setEditing] = useState(false);
  const board = useQuery({
    queryKey: qk.yardBoard,
    queryFn: () => yardApi.board(),
    refetchInterval: 30_000,
    staleTime: 15_000,
  });
  const move = useMutation({
    mutationFn: (body: { vehicle_id: number; location_id: number | null; order_id: number | null }) => yardApi.move(body),
    onSuccess: (r) => {
      if (r.over_capacity) toast('info', t('overCapacityToast'));
      void qc.invalidateQueries({ queryKey: ['yard'] });
      void qc.invalidateQueries({ queryKey: ['order'] });
    },
    onError: (e) => toast('error', errorMessage(e, ter, ter('unknownError'))),
  });

  if (board.isPending) return <DetailSkeleton />;
  if (board.isError) return <ErrorState error={board.error} onRetry={() => void board.refetch()} />;

  const { locations, vehicles } = board.data;
  const at = (id: number | null) => vehicles.filter((v) => (v.location_id ?? null) === id);
  const drop = (location_id: number | null) => (d: { vehicle_id: number; order_id: number | null }) => {
    const current = vehicles.find((v) => v.vehicle_id === d.vehicle_id);
    if ((current?.location_id ?? null) === location_id) return;
    move.mutate({ vehicle_id: d.vehicle_id, location_id, order_id: d.order_id });
  };

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="text-metadata text-steel-500">{canMove ? t('dragHint') : t('readOnly')}</p>
        {canConfigure && (
          <button type="button" className="btn-secondary btn-sm" onClick={() => setEditing((e) => !e)}>
            {editing ? t('doneEditing') : t('editPlaces')}
          </button>
        )}
      </div>
      {editing && <LocationsEditor />}
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-4">
        <Column
          title={t('unplaced')}
          subtitle={t('unplacedHint')}
          vehicles={at(null).filter((v) => v.order_id != null)}
          tone="muted"
          canMove={false}
          onDropVehicle={() => undefined}
        />
        {locations.map((l) => (
          <Column
            key={l.id}
            title={l.name}
            subtitle={lookupLabel(lookups?.yard_kinds, l.kind)}
            capacity={l.capacity}
            vehicles={at(l.id)}
            canMove={canMove}
            onDropVehicle={drop(l.id)}
          />
        ))}
        {canMove && (
          <section
            className="flex min-h-24 flex-col items-center justify-center gap-1 rounded-xl border border-dashed border-steel-200 p-3 text-center text-metadata text-steel-500"
            onDragOver={(e) => e.dataTransfer.types.includes(DRAG_TYPE) && e.preventDefault()}
            onDrop={(e) => {
              const raw = e.dataTransfer.getData(DRAG_TYPE);
              if (!raw) return;
              e.preventDefault();
              drop(null)(JSON.parse(raw));
            }}
          >
            <LogOut className="h-5 w-5" aria-hidden />
            {t('leftSite')}
          </section>
        )}
      </div>
    </div>
  );
}

function LocationsEditor() {
  const t = useTranslations('yard');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const { data: lookups } = useLookups();
  const [name, setName] = useState('');
  const [error, setError] = useState<string | null>(null);
  const list = useQuery({ queryKey: qk.yardLocations(true), queryFn: () => yardApi.locations(true) });
  const done = () => {
    setError(null);
    void qc.invalidateQueries({ queryKey: ['yard'] });
  };
  const onError = (e: unknown) => setError(errorMessage(e, ter, ter('unknownError')));
  const create = useMutation({
    mutationFn: () => yardApi.createLocation({ name: name.trim(), kind: 'bay', capacity: 1 }),
    onSuccess: () => {
      setName('');
      done();
    },
    onError,
  });
  const update = useMutation({
    mutationFn: ({ id, body }: { id: number; body: Parameters<typeof yardApi.updateLocation>[1] }) => yardApi.updateLocation(id, body),
    onSuccess: done,
    onError,
  });
  const row = (l: YardLocation) => (
    <li key={l.id} className={cn('flex flex-wrap items-center gap-2 py-1.5', l.archived_at && 'opacity-60')}>
      <input
        className="input h-8 min-w-0 flex-1 py-0"
        defaultValue={l.name}
        aria-label={t('placeName')}
        onBlur={(e) => {
          const next = e.target.value.trim();
          if (next && next !== l.name) update.mutate({ id: l.id, body: { name: next } });
        }}
      />
      <select
        className="input h-8 w-auto py-0"
        value={l.kind}
        aria-label={t('placeKind')}
        onChange={(e) => update.mutate({ id: l.id, body: { kind: e.target.value } })}
      >
        {(lookups?.yard_kinds ?? []).map((k) => (
          <option key={k.key} value={k.key}>{k.label_hu}</option>
        ))}
      </select>
      <input
        className="input h-8 w-20 py-0"
        type="number"
        min={1}
        max={100}
        defaultValue={l.capacity ?? ''}
        placeholder="∞"
        aria-label={t('capacity')}
        onBlur={(e) => {
          const v = e.target.value.trim();
          const next = v ? Number(v) : null;
          if (next !== (l.capacity ?? null)) update.mutate({ id: l.id, body: { capacity: next } });
        }}
      />
      <input
        className="input h-8 w-20 py-0"
        type="number"
        defaultValue={l.position}
        aria-label={t('position')}
        onBlur={(e) => {
          const next = Number(e.target.value);
          if (Number.isFinite(next) && next !== l.position) update.mutate({ id: l.id, body: { position: next } });
        }}
      />
      <button
        type="button"
        className="btn-ghost btn-sm"
        title={l.archived_at ? t('restore') : t('archive')}
        onClick={() => update.mutate({ id: l.id, body: { archived: !l.archived_at } })}
      >
        {l.archived_at ? <ArchiveRestore className="h-4 w-4" aria-hidden /> : <Archive className="h-4 w-4" aria-hidden />}
      </button>
    </li>
  );
  return (
    <section className="card">
      <div className="card-header">
        <h2 className="text-section font-semibold">{t('placesTitle')}</h2>
        <p className="text-metadata text-steel-500">{t('placesIntro')}</p>
      </div>
      <div className="card-content space-y-2">
        {error && <p className="text-body text-signal" role="alert">{error}</p>}
        <ul className="divide-y divide-steel-200">{(list.data?.items ?? []).map(row)}</ul>
        <form
          className="flex gap-2 border-t border-steel-200 pt-3"
          onSubmit={(e) => {
            e.preventDefault();
            if (name.trim()) create.mutate();
          }}
        >
          <input className="input h-8 min-w-0 flex-1 py-0" placeholder={t('newPlace')} value={name} onChange={(e) => setName(e.target.value)} />
          <button type="submit" className="btn-secondary btn-sm" disabled={!name.trim() || create.isPending}>
            <Plus className="h-4 w-4" aria-hidden /> {t('add')}
          </button>
        </form>
      </div>
    </section>
  );
}

/** Where an order's vehicles stand, with a select to move one (order detail). */
export function VehicleLocations({
  orderId,
  vehicles,
  locations: current,
  canMove,
}: {
  orderId: number;
  vehicles: { id: number; plate?: string | null; vin?: string | null }[];
  locations: { vehicle_id: number; location_id?: number | null; location_name?: string | null; moved_at: string; moved_by_name?: string | null }[];
  canMove: boolean;
}) {
  const t = useTranslations('yard');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const { toast } = useToast();
  const places = useQuery({ queryKey: qk.yardLocations(false), queryFn: () => yardApi.locations(false), enabled: vehicles.length > 0 });
  const move = useMutation({
    mutationFn: (body: { vehicle_id: number; location_id: number | null }) => yardApi.move({ ...body, order_id: orderId }),
    onSuccess: (r) => {
      if (r.over_capacity) toast('info', t('overCapacityToast'));
      void qc.invalidateQueries({ queryKey: ['order', orderId] });
      void qc.invalidateQueries({ queryKey: ['yard'] });
    },
    onError: (e) => toast('error', errorMessage(e, ter, ter('unknownError'))),
  });
  if (!vehicles.length) return null;
  return (
    <ul className="mt-2 space-y-1">
      {vehicles.map((v) => {
        const here = current.find((c) => c.vehicle_id === v.id);
        return (
          <li key={v.id} className="flex flex-wrap items-center gap-2 text-metadata">
            <span className="font-mono">{v.plate ?? v.vin ?? `#${v.id}`}</span>
            <span className="text-steel-500">{t('standsAt')}</span>
            {canMove ? (
              <select
                className="input h-8 w-auto py-0"
                value={here?.location_id ?? ''}
                onChange={(e) => move.mutate({ vehicle_id: v.id, location_id: e.target.value ? Number(e.target.value) : null })}
                aria-label={t('moveTo')}
              >
                <option value="">{here ? t('leftSite') : t('notPlaced')}</option>
                {(places.data?.items ?? []).map((p) => (
                  <option key={p.id} value={p.id}>{p.name}</option>
                ))}
              </select>
            ) : (
              <span className="font-medium">{here?.location_name ?? (here ? t('leftSite') : t('notPlaced'))}</span>
            )}
            {here && (
              <span className="text-steel-500">
                · <DateDisplay withTime value={here.moved_at} />
                {here.moved_by_name ? ` · ${here.moved_by_name}` : ''}
              </span>
            )}
          </li>
        );
      })}
    </ul>
  );
}
