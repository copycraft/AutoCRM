'use client';

// A record's whole history, newest first, grouped by day, the way MiniCRM showed it:
// data changes, status changes, uploaded files (with a "Megtekint" button), tasks, emails
// and notes. One component for every record that has a history.

import { useMemo, useState } from 'react';
import Link from 'next/link';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation, useQuery } from '@tanstack/react-query';
import {
  Activity,
  CheckCircle2,
  ChevronsRight,
  FilePen,
  ListTodo,
  Mail,
  Paperclip,
  Sparkles,
  StickyNote,
} from 'lucide-react';
import { incomingApi, mediaApi, timelineApi, type TimelineEntity } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { cn } from '@/lib/utils/format';
import { DayLabel, groupByDay } from '@/components/ui/DayGroups';
import { ErrorState } from '@/components/ui/ErrorState';
import type { TimelineEvent } from '@/lib/api/types';

/** The field names a change can mention, as the office calls them. */
const FIELD_LABELS: Record<string, string> = {
  title: 'Cím',
  name: 'Név',
  full_name: 'Név',
  partner_id: 'Partner',
  contact_id: 'Kapcsolattartó',
  contact_name: 'Kapcsolattartó neve',
  contact_email: 'Kapcsolattartó e-mail',
  contact_phone: 'Kapcsolattartó telefon',
  email: 'E-mail',
  phone: 'Telefon',
  company_phone: 'Céges telefon',
  personal_phone: 'Személyes telefon',
  source: 'Forrás',
  description: 'Leírás',
  assigned_to: 'Felelős',
  quoted_value_minor: 'Árajánlat összege',
  currency: 'Pénznem',
  quote_valid_until: 'Árajánlat érvényes',
  tags: 'Címkék',
  annual_leave_days: 'Éves szabadság',
  kind: 'Típus',
  supplier_name: 'Beszállító',
  invoice_number: 'Számlaszám',
  gross_amount: 'Bruttó',
  paid_amount: 'Kifizetett összeg',
  payment_method: 'Fizetési mód',
  booking_only: 'Csak könyvelésben',
  notes: 'Megjegyzés',
  tax_number: 'Adószám',
  city: 'Város',
  address_line: 'Cím',
  postal_code: 'Irányítószám',
  country: 'Ország',
  role: 'Szerepkör',
  archived: 'Archiválva',
  label: 'Állapot',
};

type Filter = 'all' | 'changes' | 'files' | 'tasks' | 'emails' | 'notes';

function filterOf(e: TimelineEvent): Exclude<Filter, 'all'> {
  if (e.kind === 'file') return 'files';
  if (e.kind === 'task') return 'tasks';
  if (e.kind === 'email') return 'emails';
  if (e.kind === 'note') return 'notes';
  return 'changes';
}

function formatValue(field: string, v: unknown): string {
  if (v === null || v === undefined || v === '') return '—';
  if (typeof v === 'boolean') return v ? 'igen' : 'nem';
  if (typeof v === 'number' && (field.endsWith('_minor') || field.endsWith('_amount'))) {
    return (v / 100).toLocaleString('hu-HU', { maximumFractionDigits: 2 });
  }
  if (Array.isArray(v)) return v.map((x) => formatValue(field, x)).join(', ') || '—';
  if (typeof v === 'object') return JSON.stringify(v);
  return String(v);
}

/** `{field: [old, new]}` or `{field: {from, to}}` → rows of (field, old, new). */
function changeRows(changes: unknown): { field: string; old?: unknown; next: unknown }[] {
  if (!changes || typeof changes !== 'object' || Array.isArray(changes)) return [];
  return Object.entries(changes as Record<string, unknown>)
    .filter(([field]) => field !== 'automatic')
    .map(([field, value]) => {
      if (Array.isArray(value) && value.length === 2) return { field, old: value[0], next: value[1] };
      if (value && typeof value === 'object' && 'to' in (value as object)) {
        const o = value as { from?: unknown; to: unknown };
        return { field, old: o.from, next: o.to };
      }
      return { field, next: value };
    });
}

export function Timeline({ entity, id }: { entity: TimelineEntity; id: number }) {
  const t = useTranslations('timeline');
  const [filter, setFilter] = useState<Filter>('all');
  const query = useQuery({ queryKey: ['timeline', entity, id], queryFn: () => timelineApi.get(entity, id) });
  const items = useMemo(() => query.data?.items ?? [], [query.data]);
  const counts = useMemo(() => {
    const c: Record<Filter, number> = { all: items.length, changes: 0, files: 0, tasks: 0, emails: 0, notes: 0 };
    for (const e of items) c[filterOf(e)] += 1;
    return c;
  }, [items]);
  const shown = filter === 'all' ? items : items.filter((e) => filterOf(e) === filter);

  if (query.isPending) return <p className="text-metadata text-steel-500">{t('loading')}</p>;
  if (query.isError) return <ErrorState error={query.error} onRetry={() => void query.refetch()} />;

  return (
    <div className="space-y-4" data-testid="timeline">
      <div className="flex flex-wrap gap-1.5" role="group" aria-label={t('filter')}>
        {(['all', 'changes', 'files', 'tasks', 'emails', 'notes'] as Filter[])
          .filter((f) => f === 'all' || counts[f] > 0)
          .map((f) => (
            <button
              key={f}
              type="button"
              aria-pressed={filter === f}
              className={cn('btn-sm rounded-full', filter === f ? 'btn-primary' : 'btn-secondary')}
              onClick={() => setFilter(f)}
            >
              {t(`filters.${f}`)} <span className="font-mono opacity-70">{counts[f]}</span>
            </button>
          ))}
      </div>
      {shown.length === 0 && <p className="text-metadata text-steel-500">{t('empty')}</p>}
      {groupByDay(shown, (e) => e.at).map((g) => (
        <div key={g.day}>
          <div className="border-b border-steel-200 pb-1">
            <DayLabel day={g.day} />
          </div>
          <ol className="mt-3">
            {g.items.map((e, i) => (
              <Event key={`${e.at}-${e.kind}-${i}`} e={e} last={i === g.items.length - 1} />
            ))}
          </ol>
        </div>
      ))}
    </div>
  );
}

const ICONS = {
  create: { Icon: Sparkles, bg: 'bg-cold' },
  change: { Icon: FilePen, bg: 'bg-cold' },
  stage: { Icon: ChevronsRight, bg: 'bg-cold' },
  file: { Icon: Paperclip, bg: 'bg-steel-500' },
  task: { Icon: ListTodo, bg: 'bg-steel-500' },
  taskDone: { Icon: CheckCircle2, bg: 'bg-done' },
  email: { Icon: Mail, bg: 'bg-steel-900' },
  note: { Icon: StickyNote, bg: 'bg-steel-500' },
  event: { Icon: Activity, bg: 'bg-cold' },
} as const;

function Event({ e, last }: { e: TimelineEvent; last: boolean }) {
  const t = useTranslations('timeline');
  const locale = useLocale();
  const icon =
    e.kind === 'task' && e.action === 'done'
      ? ICONS.taskDone
      : ICONS[(e.kind in ICONS ? e.kind : 'event') as keyof typeof ICONS];
  const time = new Intl.DateTimeFormat('hu-HU', { timeZone: 'Europe/Budapest', hour: '2-digit', minute: '2-digit' }).format(new Date(e.at));
  // Employee status moves are audit events, but read as a status change.
  const statusLabel =
    e.kind === 'stage' ? e.text : e.action === 'status' ? ((e.changes as { label?: string })?.label ?? null) : null;

  return (
    <li className="relative flex gap-4 pb-5">
      {!last && <span className="absolute left-[19px] top-11 bottom-1 w-0.5 bg-cold/40" aria-hidden />}
      <span className={cn('flex h-10 w-10 shrink-0 items-center justify-center rounded-full text-white', icon.bg)}>
        <icon.Icon className="h-5 w-5" aria-hidden />
      </span>
      <div className="min-w-0 flex-1 border-b border-steel-200 pb-4">
        <div className="flex items-baseline justify-between gap-3">
          <p className="font-semibold text-steel-900">{title(t, e)}</p>
          <span className="shrink-0 font-mono text-metadata text-steel-500">{time}</span>
        </div>
        {e.user_name && <p className="text-metadata text-steel-500">{e.user_name}</p>}

        <div className="mt-2 space-y-1 text-body">
          {statusLabel && (
            <span className="inline-block rounded-r-full bg-cold/15 py-0.5 pl-3 pr-4 font-medium text-steel-900 [clip-path:polygon(0_0,100%_0,100%_100%,0_100%,8px_50%)]">
              {statusLabel}
            </span>
          )}
          {e.kind === 'stage' && e.detail && <p className="whitespace-pre-wrap text-steel-500">{e.detail}</p>}

          {(e.kind === 'change' || e.kind === 'create' || (e.kind === 'event' && e.action !== 'status')) &&
            changeRows(e.changes).map((r) => (
              <p key={r.field} title={r.old !== undefined ? `${t('before')}: ${formatValue(r.field, r.old)}` : undefined}>
                <span className="text-steel-900">{formatValue(r.field, r.next)}</span>{' '}
                <span className="text-steel-500">({FIELD_LABELS[r.field] ?? r.field})</span>
              </p>
            ))}

          {e.kind === 'task' && <p>{e.text}</p>}
          {e.kind === 'note' && <p className="whitespace-pre-wrap">{e.text}</p>}
          {e.kind === 'email' && (
            <p>
              {e.email_id ? (
                <Link href={`/${locale}/emails/${e.email_id}`} className="underline">
                  {e.text}
                </Link>
              ) : (
                e.text
              )}
              {e.detail && <span className="text-steel-500"> → {e.detail}</span>}
            </p>
          )}
          {e.kind === 'event' && e.text && (
            <p>
              {e.order_id ? (
                <Link href={`/${locale}/orders/${e.order_id}`} className="underline">{e.text}</Link>
              ) : e.lead_id ? (
                <Link href={`/${locale}/leads/${e.lead_id}`} className="underline">{e.text}</Link>
              ) : (
                e.text
              )}
            </p>
          )}
          {e.kind === 'file' && <FileLine e={e} />}
        </div>
      </div>
    </li>
  );
}

function title(t: ReturnType<typeof useTranslations>, e: TimelineEvent): string {
  switch (e.kind) {
    case 'create':
      return t('create');
    case 'change':
      return t('change');
    case 'stage':
      return t('stage');
    case 'file':
      return t('file');
    case 'note':
      return t('note');
    case 'task':
      return e.action === 'done' ? t('taskDone') : t('taskCreated');
    case 'email':
      return ['sent', 'failed', 'queued', 'cancelled', 'needs_review', 'sending'].includes(e.action)
        ? t(`email.${e.action}`)
        : t('email.sent');
    default: {
      const known = [
        'status', 'archive', 'unarchive', 'convert', 'order_created', 'lead_created', 'invoice_issue',
        'invoice_storno', 'invoice_annul', 'invoice_annulled', 'invoice_rejected', 'invoice_reconciled',
        'proforma_created', 'paid', 'unpaid', 'photo_set', 'photo_removed', 'delete', 'item_add',
        'item_update', 'item_remove', 'spec_set', 'blocker_nudge', 'import',
      ];
      return known.includes(e.action) ? t(`actions.${e.action}`) : e.action.replace(/_/g, ' ');
    }
  }
}

function FileLine({ e }: { e: TimelineEvent }) {
  const t = useTranslations('timeline');
  const ter = useTranslations('errors');
  const open = useMutation({
    mutationFn: async () => {
      if (e.document_id) return (await mediaApi.downloadDocument(e.document_id)).url;
      if (e.image_id) return (await mediaApi.original(e.image_id)).url;
      if (e.incoming_invoice_id) return (await incomingApi.fileUrl(e.incoming_invoice_id)).url;
      throw new Error('no file');
    },
    onSuccess: (url) => window.open(url, '_blank', 'noopener'),
  });
  const size = e.file_size != null ? `${(e.file_size / (1024 * 1024)).toFixed(2)} MB` : null;
  return (
    <div className="space-y-2">
      <p className="flex flex-wrap items-center gap-1.5">
        <Paperclip className="h-4 w-4 text-steel-500" aria-hidden />
        <span className="font-medium">{e.file_name ?? t('unnamedFile')}</span>
        {size && <span className="text-steel-500">({size})</span>}
      </p>
      <button type="button" className="btn-secondary btn-sm" disabled={open.isPending} onClick={() => open.mutate()}>
        {t('view')}
      </button>
      {open.isError && (
        <p className="text-metadata text-signal" role="alert">
          {errorMessage(open.error, ter, ter('unknownError'))}
        </p>
      )}
    </div>
  );
}
