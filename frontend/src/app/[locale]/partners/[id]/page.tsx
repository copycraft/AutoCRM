'use client';

import Link from 'next/link';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { DetailSkeleton } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { ConfirmDialog } from '@/components/ui/ConfirmDialog';
import { PartnerForm, partnerPatchBody, type PartnerFormValues } from '@/components/forms/PartnerForm';
import { ContactSection } from '@/components/forms/ContactSection';
import { Money } from '@/components/ui/Money';
import { partnersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { canEditPartners, useAuth } from '@/lib/auth/context';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { useState } from 'react';

function DetailRow({ label, value, mono }: { label: string; value: React.ReactNode; mono?: boolean }) {
  return (
    <div className="flex flex-col gap-0.5">
      <dt className="text-metadata font-medium text-steel-500">{label}</dt>
      <dd className={`text-sm ${mono ? 'font-mono' : ''}`}>{value}</dd>
    </div>
  );
}

export default function PartnerDetailPage({ params }: { params: { id: string } }) {
  const id = Number(params.id);
  const t = useTranslations('partners');
  const tc = useTranslations('common');
  const tn = useTranslations('navigation');
  const locale = useLocale();
  const { user } = useAuth();
  const qc = useQueryClient();
  const [editing, setEditing] = useState(false);
  const [confirmArchive, setConfirmArchive] = useState(false);
  const editable = canEditPartners(user);

  const detail = useQuery({ queryKey: qk.partner(id), queryFn: () => partnersApi.get(id) });

  const patch = useMutation({
    mutationFn: (body: Record<string, unknown>) => partnersApi.patch(id, body),
    onSuccess: () => {
      setEditing(false);
      void qc.invalidateQueries({ queryKey: qk.partner(id) });
      void qc.invalidateQueries({ queryKey: ['partners'] });
    },
  });

  const setArchived = useMutation({
    mutationFn: (archived: boolean) =>
      archived ? partnersApi.archive(id) : partnersApi.unarchive(id),
    onSuccess: () => {
      setConfirmArchive(false);
      void qc.invalidateQueries({ queryKey: qk.partner(id) });
      void qc.invalidateQueries({ queryKey: ['partners'] });
    },
  });

  return (
    <AppShell>
      {detail.isLoading ? (
        <DetailSkeleton />
      ) : detail.isError || !detail.data ? (
        <>
          <PageHeader title={t('partnerDetails')} />
          <ErrorState error={detail.error} onRetry={() => void detail.refetch()} />
        </>
      ) : (
        (() => {
          const { partner, contacts, orders } = detail.data;
          const archived = partner.archived_at !== null;
          return (
            <>
              <PageHeader
                title={partner.name}
                subtitle={`#${partner.id} · ${partner.kind === 'business' ? t('business') : t('person')}`}
                actions={editable && (
                  <>
                    {!editing && (
                      <button className="btn-secondary btn-sm" onClick={() => setEditing(true)}>
                        {tc('edit')}
                      </button>
                    )}
                    <button
                      className="btn-ghost btn-sm"
                      onClick={() => setConfirmArchive(true)}
                      disabled={setArchived.isPending}
                    >
                      {archived ? t('unarchive') : t('archive')}
                    </button>
                  </>
                )}
              />

              {archived && (
                <p>
                  <StatusBadge tone="steel">{t('archived')}</StatusBadge>
                </p>
              )}

              {editing ? (
                <PartnerForm
                  initial={partner}
                  submitLabel={tc('save')}
                  onSubmit={(v: PartnerFormValues) =>
                    patch.mutateAsync(partnerPatchBody(partner, v)).then(() => undefined)
                  }
                />
              ) : (
                <section className="card">
                  <div className="card-content grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
                    <DetailRow label={t('taxNumber')} value={partner.tax_number ?? '—'} mono />
                    <DetailRow label={t('euTaxNumber')} value={partner.eu_tax_number ?? '—'} mono />
                    <DetailRow label={t('country')} value={partner.country} mono />
                    <DetailRow label={t('defaultCurrency')} value={partner.default_currency} mono />
                    <DetailRow label={tc('email')} value={partner.email ?? '—'} />
                    <DetailRow label={tc('phone')} value={partner.phone ?? '—'} />
                    <DetailRow label={t('website')} value={partner.website ?? '—'} />
                    <DetailRow
                      label={t('addressLine')}
                      value={[partner.postal_code, partner.city, partner.address_line].filter(Boolean).join(' · ') || '—'}
                    />
                    <DetailRow label={t('notes')} value={partner.notes ?? '—'} />
                    <DetailRow label={t('createdAt')} value={<DateDisplay value={partner.created_at} />} />
                  </div>
                </section>
              )}

              {editable ? (
                <ContactSection partnerId={id} contacts={contacts} />
              ) : (
                <section className="card">
                  <div className="card-header">
                    <h2 className="text-section font-semibold">{t('contacts')} ({contacts.length})</h2>
                  </div>
                  <div className="card-content space-y-2">
                    {contacts.map((c) => (
                      <p key={c.id} className="text-sm">
                        <span className="font-medium">{c.name}</span>
                        <span className="text-steel-500">
                          {' '}
                          · {[c.position, c.email, c.phone].filter(Boolean).join(' · ')}
                        </span>
                      </p>
                    ))}
                    {contacts.length === 0 && <p className="text-sm text-steel-500">—</p>}
                  </div>
                </section>
              )}

              <section className="card">
                <div className="card-header">
                  <h2 className="text-section font-semibold">
                    {tn('orders')} ({orders.length})
                  </h2>
                </div>
                <div className="card-content space-y-2">
                  {orders.map((o) => (
                    <Link
                      key={o.id}
                      href={`/${locale}/orders/${o.id}`}
                      className="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-steel-200 px-3 py-2 hover:bg-panel"
                    >
                      <span className="text-sm">
                        <span className="font-mono font-medium">{o.number}</span>{' '}
                        <span className="font-medium">{o.title}</span>{' '}
                        <span className="text-steel-500">· {o.stage_label}</span>
                      </span>
                      <Money minor={o.total_minor} currency={o.currency} />
                    </Link>
                  ))}
                  {orders.length === 0 && (
                    <p className="text-sm text-steel-500">{t('noOrders')}</p>
                  )}
                </div>
              </section>

              <ConfirmDialog
                open={confirmArchive}
                title={archived ? t('unarchive') : t('archive')}
                body={archived ? undefined : t('archiveBody')}
                confirmLabel={archived ? t('unarchive') : t('archive')}
                onClose={() => setConfirmArchive(false)}
                busy={setArchived.isPending}
                onConfirm={() => setArchived.mutate(!archived)}
              />
            </>
          );
        })()
      )}
    </AppShell>
  );
}
