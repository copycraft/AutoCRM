'use client';

// Bejövő számlák: supplier invoices. Drop the files anywhere on the page (or pick them);
// each one becomes an open invoice and its form opens so the office can type in what it
// says. The list an invoice is on — open, paid by transfer, cash, partial... — follows
// from those figures; nobody files it by hand.

import { useMemo, useRef, useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { ExternalLink, FileUp, Trash2 } from 'lucide-react';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { Pagination } from '@/components/ui/Pagination';
import { Money } from '@/components/ui/Money';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { ErrorState } from '@/components/ui/ErrorState';
import { ConfirmDialog } from '@/components/ui/ConfirmDialog';
import { BucketBadge, BucketList, type Bucket } from '@/components/tags/BucketList';
import { Timeline } from '@/components/timeline/Timeline';
import { ExportMenu, collectAll } from '@/components/tables/ExportCsvButton';
import { SavedViewsBar } from '@/components/tables/SavedViewsBar';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { useUrlInt, useUrlState } from '@/hooks/useUrlState';
import { incomingApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { canIssueInvoices, useAuth } from '@/lib/auth/context';
import { cn, minorToMajorString, parseMajorToMinor } from '@/lib/utils/format';
import type { Currency, IncomingInvoice } from '@/lib/api/types';

const PAGE = 50;

function useIncomingBuckets(): { title: string; buckets: Bucket[] }[] {
  const t = useTranslations('incoming.buckets');
  return [
    {
      title: t('groupOpen'),
      buckets: [
        { key: 'open_invoice', label: t('open_invoice'), color: '#1f3a75' },
        { key: 'open_proforma', label: t('open_proforma'), color: '#dbe8ff' },
      ],
    },
    {
      title: t('groupDone'),
      buckets: [
        { key: 'transferred', label: t('transferred'), color: '#4f8a52' },
        { key: 'cash', label: t('cash'), color: '#1e3320' },
      ],
    },
    {
      title: t('groupPartial'),
      buckets: [
        { key: 'partial', label: t('partial'), color: '#f2a33a' },
        { key: 'cash_receipt', label: t('cash_receipt'), color: '#5b3a14' },
        { key: 'booking_only', label: t('booking_only'), color: '#1f2228' },
      ],
    },
  ];
}

/** Today in Budapest as YYYY-MM-DD. */
function today(): string {
  return new Intl.DateTimeFormat('en-CA', { timeZone: 'Europe/Budapest' }).format(new Date());
}

export default function IncomingInvoicesPage() {
  const t = useTranslations('incoming');
  const tn = useTranslations('navigation');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const { user } = useAuth();
  const editable = canIssueInvoices(user);
  const qc = useQueryClient();
  const groups = useIncomingBuckets();
  const allBuckets = groups.flatMap((g) => g.buckets);

  const [q, setQ] = useUrlState('q', '');
  const [bucket, setBucket] = useUrlState('bucket', '');
  const [page, setPage] = useUrlInt('page', 1);
  const debouncedQ = useDebouncedValue(q);
  const offset = Math.max(0, (page - 1) * PAGE);

  const [open, setOpen] = useState<number | null>(null);
  const [uploads, setUploads] = useState<{ name: string; state: 'busy' | 'done' | 'error'; message?: string }[]>([]);
  const [dragging, setDragging] = useState(false);
  const [removing, setRemoving] = useState<IncomingInvoice | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);

  const list = useQuery({
    queryKey: ['incoming-invoices', { q: debouncedQ, bucket, offset }],
    queryFn: () =>
      incomingApi.list({ q: debouncedQ || undefined, bucket: bucket || undefined, limit: PAGE, offset }),
  });
  const counts = useQuery({ queryKey: ['incoming-invoices', 'buckets'], queryFn: () => incomingApi.buckets() });
  const countMap = Object.fromEntries((counts.data?.items ?? []).map((c) => [c.bucket, c.count]));
  const refresh = () => void qc.invalidateQueries({ queryKey: ['incoming-invoices'] });

  const remove = useMutation({
    mutationFn: (id: number) => incomingApi.remove(id),
    onSuccess: () => {
      setRemoving(null);
      refresh();
    },
  });

  const uploadAll = async (files: File[]) => {
    if (!editable || files.length === 0) return;
    setUploads(files.map((f) => ({ name: f.name, state: 'busy' })));
    let last: number | null = null;
    for (const [i, file] of files.entries()) {
      try {
        const row = await incomingApi.upload(file);
        last = row.id;
        setUploads((u) => u.map((x, j) => (j === i ? { ...x, state: 'done' } : x)));
      } catch (e) {
        const message = errorMessage(e, ter, ter('unknownError'));
        setUploads((u) => u.map((x, j) => (j === i ? { ...x, state: 'error', message } : x)));
      }
    }
    // The newest upload lands at the top with its form open, ready to type in.
    setBucket('');
    setPage(1);
    if (last !== null) setOpen(last);
    refresh();
  };

  return (
    <AppShell>
      <div
        className="relative"
        onDragOver={(e) => {
          if (!editable) return;
          e.preventDefault();
          setDragging(true);
        }}
        onDragLeave={(e) => {
          if (e.currentTarget === e.target) setDragging(false);
        }}
        onDrop={(e) => {
          e.preventDefault();
          setDragging(false);
          void uploadAll(Array.from(e.dataTransfer.files));
        }}
      >
        {dragging && (
          <div className="pointer-events-none absolute inset-0 z-30 flex items-center justify-center rounded-xl border-2 border-dashed border-steel-500 bg-surface/80 text-section font-semibold">
            {t('dropHere')}
          </div>
        )}
        <PageHeader
          title={tn('incoming')}
          subtitle={t('subtitle')}
          actions={
            editable && (
              <>
                <input
                  ref={fileInput}
                  type="file"
                  multiple
                  accept="application/pdf,image/jpeg,image/png,image/webp,.xml,text/xml,application/xml"
                  className="hidden"
                  data-testid="incoming-file"
                  onChange={(e) => {
                    void uploadAll(Array.from(e.target.files ?? []));
                    e.target.value = '';
                  }}
                />
                <button type="button" className="btn-primary btn-sm" onClick={() => fileInput.current?.click()}>
                  <FileUp className="h-4 w-4" aria-hidden />
                  {t('upload')}
                </button>
              </>
            )
          }
        />

        <div className="mb-4">
          <SavedViewsBar listKey="incoming_invoices" />
        </div>

        {uploads.length > 0 && (
          <ul className="mb-4 space-y-1 rounded-lg bg-steel-200/50 px-3 py-2 text-body" role="status">
            {uploads.map((u, i) => (
              <li key={i} className="flex flex-wrap gap-2">
                <span className="font-medium">{u.name}</span>
                <span className={cn(u.state === 'error' ? 'text-signal' : 'text-steel-500')}>
                  {u.state === 'busy' ? t('uploading') : u.state === 'done' ? t('uploaded') : u.message}
                </span>
              </li>
            ))}
          </ul>
        )}

        <div className="grid grid-cols-1 gap-6 lg:grid-cols-[15rem_minmax(0,1fr)]">
          <aside className="card self-start p-3">
            <BucketList
              title={tn('incoming')}
              groups={groups}
              counts={countMap}
              value={bucket}
              onChange={(b) => {
                setBucket(b);
                setPage(1);
              }}
              allLabel={t('all')}
            />
          </aside>

          <div className="min-w-0 space-y-3">
            <div className="flex items-center gap-2">
              <input
                className="input"
                placeholder={t('searchPlaceholder')}
                aria-label={tc('search')}
                value={q}
                onChange={(e) => {
                  setQ(e.target.value);
                  setPage(1);
                }}
              />
              <ExportMenu
                base="bejovo-szamlak"
                onExport={async () => {
                  const all = await collectAll((offset, limit) =>
                    incomingApi.list({ q: debouncedQ || undefined, bucket: bucket || undefined, limit, offset }),
                  );
                  const money = (v: number | null | undefined) => (v == null ? null : v / 100);
                  return {
                    header: [
                      t('export.list'), t('kind'), t('supplier'), t('taxNumber'), t('number'),
                      t('issueDate'), t('dueDate'), t('net'), t('vat'), t('gross'), t('currency'),
                      t('paymentMethod'), t('paidAmount'), t('paidOn'), t('notes'), t('export.file'),
                    ],
                    rows: all.map((i) => [
                      allBuckets.find((b) => b.key === i.bucket)?.label ?? i.bucket,
                      i.kind === 'proforma' ? t('kindProforma') : i.kind === 'receipt' ? t('kindReceipt') : t('kindInvoice'),
                      i.supplier_name, i.supplier_tax_number, i.invoice_number, i.issue_date, i.due_date,
                      money(i.net_amount), money(i.vat_amount), money(i.gross_amount), i.currency,
                      i.payment_method, money(i.paid_amount), i.paid_on, i.notes, i.file_name,
                    ]),
                    count: all.length,
                  };
                }}
              />
            </div>
            {editable && (list.data?.items.length ?? 0) === 0 && !list.isPending && !bucket && !q && (
              <button
                type="button"
                className="w-full rounded-xl border-2 border-dashed border-steel-200 p-10 text-center text-body text-steel-500 hover:border-steel-500"
                onClick={() => fileInput.current?.click()}
              >
                {t('emptyDrop')}
              </button>
            )}
            {list.isError ? (
              <ErrorState error={list.error} onRetry={() => void list.refetch()} />
            ) : (
              <ul className="space-y-2">
                {(list.data?.items ?? []).map((inv) => (
                  <IncomingRow
                    key={inv.id}
                    inv={inv}
                    bucket={allBuckets.find((b) => b.key === inv.bucket)}
                    editable={editable}
                    expanded={open === inv.id}
                    onToggle={() => setOpen(open === inv.id ? null : inv.id)}
                    onSaved={refresh}
                    onRemove={() => setRemoving(inv)}
                  />
                ))}
                {!list.isPending && (list.data?.items.length ?? 0) === 0 && (bucket || q) && (
                  <li className="text-body text-steel-500">{t('empty')}</li>
                )}
              </ul>
            )}
            <Pagination
              offset={offset}
              limit={PAGE}
              loaded={list.data?.items.length ?? 0}
              onPrev={() => setPage(Math.max(1, page - 1))}
              onNext={() => setPage(page + 1)}
              onJump={setPage}
            />
          </div>
        </div>
      </div>

      <ConfirmDialog
        open={removing !== null}
        title={t('removeTitle')}
        body={t('removeBody')}
        confirmLabel={tc('delete')}
        onClose={() => setRemoving(null)}
        busy={remove.isPending}
        onConfirm={() => removing && remove.mutate(removing.id)}
      />
    </AppShell>
  );
}

function IncomingRow({
  inv,
  bucket,
  editable,
  expanded,
  onToggle,
  onSaved,
  onRemove,
}: {
  inv: IncomingInvoice;
  bucket: Bucket | undefined;
  editable: boolean;
  expanded: boolean;
  onToggle: () => void;
  onSaved: () => void;
  onRemove: () => void;
}) {
  const t = useTranslations('incoming');
  const ter = useTranslations('errors');
  const currency = inv.currency as Currency;
  const unpaid = inv.gross_amount != null && inv.paid_amount < inv.gross_amount && !inv.booking_only && inv.kind !== 'receipt';
  const overdue = unpaid && inv.due_date != null && inv.due_date < today();
  const pay = useMutation({
    mutationFn: () => incomingApi.update(inv.id, { paid_amount: inv.gross_amount ?? 0, paid_on: today() }),
    onSuccess: onSaved,
  });
  const open = useMutation({
    mutationFn: () => incomingApi.fileUrl(inv.id),
    onSuccess: (r) => window.open(r.url, '_blank', 'noopener'),
  });
  return (
    <li className={cn('card', expanded && 'ring-2 ring-steel-500/30')}>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1 p-4">
        <BucketBadge bucket={bucket} />
        <button type="button" className="min-w-0 text-left font-medium hover:underline" onClick={onToggle}>
          {inv.supplier_name || <span className="italic text-steel-500">{t('toFill')}</span>}
        </button>
        {inv.invoice_number && <span className="font-mono text-metadata text-steel-500">{inv.invoice_number}</span>}
        <span className="ml-auto flex items-center gap-3">
          {inv.gross_amount != null && <Money minor={inv.gross_amount} currency={currency} />}
          {inv.paid_amount > 0 && inv.gross_amount != null && inv.paid_amount < inv.gross_amount && (
            <span className="text-metadata text-steel-500">
              {t('paidPart')} <Money minor={inv.paid_amount} currency={currency} />
            </span>
          )}
        </span>
        <div className="flex w-full flex-wrap items-center gap-x-3 gap-y-1 text-metadata text-steel-500">
          {inv.issue_date && <span>{t('issued')} <DateDisplay value={inv.issue_date} /></span>}
          {inv.due_date && (
            <span className={cn(overdue && 'font-semibold text-signal')}>
              {t('due')} <DateDisplay value={inv.due_date} />
            </span>
          )}
          {inv.file_name && (
            <button type="button" className="inline-flex items-center gap-1 underline hover:text-steel-900" onClick={() => open.mutate()}>
              <ExternalLink className="h-3 w-3" aria-hidden />
              {inv.file_name}
            </button>
          )}
          {editable && unpaid && inv.gross_amount! > 0 && (
            <button type="button" className="underline hover:text-steel-900" disabled={pay.isPending} onClick={() => pay.mutate()}>
              {t('markPaid')}
            </button>
          )}
          {editable && (
            <button type="button" className="underline hover:text-steel-900" onClick={onToggle}>
              {expanded ? t('close') : t('edit')}
            </button>
          )}
          {editable && (
            <button type="button" className="ml-auto" aria-label={t('removeTitle')} onClick={onRemove}>
              <Trash2 className="h-4 w-4" aria-hidden />
            </button>
          )}
        </div>
        {(pay.isError || open.isError) && (
          <p className="w-full text-metadata text-signal" role="alert">
            {errorMessage(pay.error ?? open.error, ter, ter('unknownError'))}
          </p>
        )}
      </div>
      {expanded && editable && <IncomingForm key={inv.updated_at} inv={inv} onSaved={onSaved} />}
      {expanded && (
        <div className="border-t border-steel-200 p-4">
          <Timeline key={inv.updated_at} entity="incoming_invoice" id={inv.id} />
        </div>
      )}
    </li>
  );
}

function IncomingForm({ inv, onSaved }: { inv: IncomingInvoice; onSaved: () => void }) {
  const t = useTranslations('incoming');
  const ter = useTranslations('errors');
  const suppliers = useQuery({ queryKey: ['incoming-invoices', 'suppliers'], queryFn: () => incomingApi.suppliers() });
  const money = (v: number | null | undefined) => (v == null ? '' : minorToMajorString(v));
  const [f, setF] = useState({
    kind: inv.kind,
    supplier_name: inv.supplier_name,
    supplier_tax_number: inv.supplier_tax_number ?? '',
    invoice_number: inv.invoice_number ?? '',
    issue_date: inv.issue_date ?? '',
    due_date: inv.due_date ?? '',
    currency: inv.currency,
    net: money(inv.net_amount),
    vat: money(inv.vat_amount),
    gross: money(inv.gross_amount),
    payment_method: inv.payment_method,
    paid: money(inv.paid_amount || null),
    paid_on: inv.paid_on ?? '',
    booking_only: inv.booking_only,
    notes: inv.notes ?? '',
  });
  const [error, setError] = useState<string | null>(null);
  const set = <K extends keyof typeof f>(k: K, v: (typeof f)[K]) => setF((x) => ({ ...x, [k]: v }));
  const known = useMemo(() => suppliers.data?.items ?? [], [suppliers.data]);

  const pickSupplier = (name: string) => {
    set('supplier_name', name);
    const s = known.find((x) => x.supplier_name.toLowerCase() === name.trim().toLowerCase());
    if (s) {
      setF((x) => ({
        ...x,
        supplier_name: s.supplier_name,
        supplier_tax_number: x.supplier_tax_number || s.supplier_tax_number || '',
        payment_method: s.payment_method,
        currency: s.currency,
      }));
    }
  };

  const save = useMutation({
    mutationFn: () => {
      const amount = (label: string, s: string): number | null => {
        if (!s.trim()) return null;
        const v = parseMajorToMinor(s);
        if (v === null) throw new Error(t('badAmount', { field: label }));
        return v;
      };
      let gross = amount(t('gross'), f.gross);
      const net = amount(t('net'), f.net);
      const vat = amount(t('vat'), f.vat);
      // Net and VAT typed, gross left empty: it is their sum.
      if (gross === null && net !== null && vat !== null) gross = net + vat;
      return incomingApi.update(inv.id, {
        kind: f.kind,
        supplier_name: f.supplier_name,
        supplier_tax_number: f.supplier_tax_number || null,
        invoice_number: f.invoice_number || null,
        issue_date: f.issue_date || null,
        due_date: f.due_date || null,
        currency: f.currency,
        net_amount: net,
        vat_amount: vat,
        gross_amount: gross,
        payment_method: f.payment_method,
        paid_amount: amount(t('paidAmount'), f.paid) ?? 0,
        paid_on: f.paid_on || null,
        booking_only: f.booking_only,
        notes: f.notes || null,
      });
    },
    onSuccess: () => {
      setError(null);
      onSaved();
    },
    onError: (e) => setError(e instanceof Error && !('code' in e) ? e.message : errorMessage(e, ter, ter('unknownError'))),
  });

  const field = (id: string, label: string, input: React.ReactNode) => (
    <div>
      <label className="label" htmlFor={`inc-${inv.id}-${id}`}>{label}</label>
      {input}
    </div>
  );
  const id = (k: string) => `inc-${inv.id}-${k}`;

  return (
    <form
      className="grid grid-cols-1 gap-3 border-t border-steel-200 p-4 sm:grid-cols-2 lg:grid-cols-4"
      onSubmit={(e) => {
        e.preventDefault();
        save.mutate();
      }}
    >
      {field('kind', t('kind'), (
        <select id={id('kind')} className="input" value={f.kind} onChange={(e) => set('kind', e.target.value)}>
          <option value="invoice">{t('kindInvoice')}</option>
          <option value="proforma">{t('kindProforma')}</option>
          <option value="receipt">{t('kindReceipt')}</option>
        </select>
      ))}
      {field('supplier', t('supplier'), (
        <>
          <input id={id('supplier')} className="input" list={id('suppliers')} value={f.supplier_name} onChange={(e) => pickSupplier(e.target.value)} />
          <datalist id={id('suppliers')}>
            {known.map((s) => (
              <option key={s.supplier_name} value={s.supplier_name} />
            ))}
          </datalist>
        </>
      ))}
      {field('tax', t('taxNumber'), (
        <input id={id('tax')} className="input font-mono" value={f.supplier_tax_number} onChange={(e) => set('supplier_tax_number', e.target.value)} />
      ))}
      {field('number', t('number'), (
        <input id={id('number')} className="input font-mono" value={f.invoice_number} onChange={(e) => set('invoice_number', e.target.value)} />
      ))}
      {field('issue', t('issueDate'), (
        <input id={id('issue')} type="date" className="input" value={f.issue_date} onChange={(e) => set('issue_date', e.target.value)} />
      ))}
      {field('due', t('dueDate'), (
        <input id={id('due')} type="date" className="input" value={f.due_date} onChange={(e) => set('due_date', e.target.value)} />
      ))}
      {field('currency', t('currency'), (
        <select id={id('currency')} className="input" value={f.currency} onChange={(e) => set('currency', e.target.value)}>
          <option value="HUF">HUF</option>
          <option value="EUR">EUR</option>
        </select>
      ))}
      {field('method', t('paymentMethod'), (
        <select id={id('method')} className="input" value={f.payment_method} onChange={(e) => set('payment_method', e.target.value)}>
          <option value="TRANSFER">{t('transfer')}</option>
          <option value="CASH">{t('cashMethod')}</option>
          <option value="CARD">{t('card')}</option>
        </select>
      ))}
      {field('net', t('net'), (
        <input id={id('net')} className="input font-mono" inputMode="decimal" value={f.net} onChange={(e) => set('net', e.target.value)} />
      ))}
      {field('vat', t('vat'), (
        <input id={id('vat')} className="input font-mono" inputMode="decimal" value={f.vat} onChange={(e) => set('vat', e.target.value)} />
      ))}
      {field('gross', t('gross'), (
        <input id={id('gross')} className="input font-mono" inputMode="decimal" placeholder={t('grossHint')} value={f.gross} onChange={(e) => set('gross', e.target.value)} />
      ))}
      {field('paid', t('paidAmount'), (
        <div className="flex gap-1">
          <input id={id('paid')} className="input font-mono" inputMode="decimal" value={f.paid} onChange={(e) => set('paid', e.target.value)} />
          <button
            type="button"
            className="btn-secondary btn-sm shrink-0"
            disabled={!f.gross.trim()}
            onClick={() => setF((x) => ({ ...x, paid: x.gross, paid_on: x.paid_on || today() }))}
          >
            {t('paidFull')}
          </button>
        </div>
      ))}
      {field('paidOn', t('paidOn'), (
        <input id={id('paidOn')} type="date" className="input" value={f.paid_on} onChange={(e) => set('paid_on', e.target.value)} />
      ))}
      <label className="flex items-center gap-2 self-end pb-2 text-body">
        <input type="checkbox" className="rounded border-steel-200 accent-steel-900" checked={f.booking_only} onChange={(e) => set('booking_only', e.target.checked)} />
        {t('bookingOnly')}
      </label>
      <div className="sm:col-span-2">
        <label className="label" htmlFor={id('notes')}>{t('notes')}</label>
        <input id={id('notes')} className="input" value={f.notes} onChange={(e) => set('notes', e.target.value)} />
      </div>
      <div className="flex items-end justify-end gap-2 sm:col-span-2 lg:col-span-4">
        {error && <p className="mr-auto text-body text-signal" role="alert">{error}</p>}
        <button className="btn-primary btn-sm" type="submit" disabled={save.isPending}>
          {save.isPending ? '…' : t('save')}
        </button>
      </div>
    </form>
  );
}
