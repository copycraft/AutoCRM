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
import { canEditPartners, canSendEmail, useAuth } from '@/lib/auth/context';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { TaskList } from '@/components/tasks/TaskList';
import { Timeline } from '@/components/timeline/Timeline';
import { ContactLine, EmailValue, PhoneValue } from '@/components/ui/ContactLinks';
import { useToast } from '@/components/ui/Toasts';
import { Breadcrumbs, BackToList } from '@/components/ui/Breadcrumbs';
import { CopyButton, CopyLinkButton } from '@/components/ui/CopyButton';
import { RawImportPanel } from '@/components/migration/RawImportPanel';
import { InvoiceLanguageSelect } from '@/components/partners/InvoiceLanguageSelect';
import { useRecentRecords } from '@/hooks/useRecent';
import { useEffect, useState } from 'react';

function DetailRow({ label, value, mono }: { label: string; value: React.ReactNode; mono?: boolean }) {
  return (
    <div className="flex flex-col gap-0.5">
      <dt className="text-metadata font-medium text-steel-500">{label}</dt>
      <dd className={`text-body ${mono ? 'font-mono' : ''}`}>{value}</dd>
    </div>
  );
}

export default function PartnerDetailPage({ params }: { params: { id: string } }) {
  const id = Number(params.id);
  const t = useTranslations('partners');
  const tc = useTranslations('common');
  const tn = useTranslations('navigation');
  const tt = useTranslations('tasks');
  const tl = useTranslations('timeline');
  const locale = useLocale();
  const { user } = useAuth();
  const qc = useQueryClient();
  const { push: pushRecent } = useRecentRecords(6);
  const toast = useToast();
  const tq = useTranslations('qol');
  const ts = useTranslations('statement');
  const [editing, setEditing] = useState(false);
  const [confirmArchive, setConfirmArchive] = useState(false);
  const editable = canEditPartners(user);

  const detail = useQuery({ queryKey: qk.partner(id), queryFn: () => partnersApi.get(id) });
  useEffect(() => {
    if (detail.data) {
      pushRecent({
        href: `/${locale}/partners/${id}`,
        title: detail.data.partner.name,
        sub: detail.data.partner.city ?? undefined,
      });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [detail.data?.partner.id]);

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
    onSuccess: (_d, archived) => {
      setConfirmArchive(false);
      void qc.invalidateQueries({ queryKey: qk.partner(id) });
      void qc.invalidateQueries({ queryKey: ['partners'] });
      toast.success(archived ? t('archived') : t('unarchive'), undefined, {
        label: tq('undo'),
        onClick: () => setArchived.mutate(!archived),
      });
    },
  });

  return (
    <AppShell>
      {detail.isLoading ? (
        <DetailSkeleton />
      ) : detail.isError || !detail.data ? (
        <>
          <PageHeader size="record" title={t('partnerDetails')} />
          <ErrorState error={detail.error} onRetry={() => void detail.refetch()} />
        </>
      ) : (
        (() => {
          const { partner, contacts, orders } = detail.data;
          const archived = partner.archived_at !== null;
          return (
            <>
              <BackToList
                listKey="partners"
                fallbackHref={`/${locale}/partners/${partner.kind === 'business' ? 'business' : 'consumers'}`}
              />
              <div className="flex items-center justify-between gap-3">
                <Breadcrumbs
                  items={[
                    {
                      href: `/${locale}/partners/${partner.kind === 'business' ? 'business' : 'consumers'}`,
                      label: tn('partners'),
                    },
                    { label: partner.name },
                  ]}
                />
                <CopyLinkButton />
              </div>
              <PageHeader size="record"
                title={
                  <span className="inline-flex items-center gap-2">
                    <span>{partner.name}</span>
                    <CopyButton value={partner.name} label={tc('name')} />
                    {partner.email && <CopyButton value={partner.email} label={tc('email')} />}
                    {partner.phone && <CopyButton value={partner.phone} label={tc('phone')} />}
                  </span>
                }
                subtitle={`#${partner.id} · ${partner.kind === 'business' ? t('business') : t('person')}`}
                actions={(
                  <>
                    <Link className="btn-secondary btn-sm" href={`/${locale}/partners/${partner.id}/statement`}>
                      {ts('open')}
                    </Link>
                    {editable && (<>
                    {!editing && (
                      <button className="btn-secondary btn-sm" onClick={() => setEditing(true)}>
                        {tc('edit')}
                      </button>
                    )}
                    {canSendEmail(user) && (
                      <Link
                        className="btn-secondary btn-sm"
                        href={`/${locale}/emails/new?partner_id=${partner.id}${partner.email ? `&to=${encodeURIComponent(partner.email)}` : ''}`}
                      >
                        {t('writeEmail')}
                      </Link>
                    )}
                    <button
                      className="btn-ghost btn-sm"
                      onClick={() => setConfirmArchive(true)}
                      disabled={setArchived.isPending}
                    >
                      {archived ? t('unarchive') : t('archive')}
                    </button>
                    </>)}
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
                <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
                  <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
                    <DetailRow label={t('taxNumber')} value={partner.tax_number ?? '—'} mono />
                    <DetailRow label={t('euTaxNumber')} value={partner.eu_tax_number ?? '—'} mono />
                    <DetailRow label={t('country')} value={partner.country} mono />
                    <DetailRow label={t('defaultCurrency')} value={partner.default_currency} mono />
                    <DetailRow label={tc('email')} value={<EmailValue value={partner.email} />} />
                    <DetailRow label={tc('phone')} value={<PhoneValue value={partner.phone} mono />} />
                    <DetailRow label={t('website')} value={partner.website ?? '—'} />
                    <DetailRow
                      label={t('addressLine')}
                      value={
                        partner.city || partner.address_line ? (
                          <a
                            className="underline"
                            target="_blank"
                            rel="noreferrer"
                            href={`https://www.google.com/maps/dir/?api=1&destination=${encodeURIComponent(
                              [partner.postal_code, partner.city, partner.address_line, partner.country].filter(Boolean).join(' '),
                            )}`}
                          >
                            {[partner.postal_code, partner.city, partner.address_line].filter(Boolean).join(' · ')}
                          </a>
                        ) : (
                          '—'
                        )
                      }
                    />
                    <DetailRow
                      label={ts('invoiceLanguage')}
                      value={<InvoiceLanguageSelect partnerId={partner.id} value={partner.invoice_language} editable={editable} />}
                    />
                    <DetailRow label={t('notes')} value={partner.notes ?? '—'} />
                    <DetailRow label={t('createdAt')} value={<DateDisplay value={partner.created_at} />} />
                  </div>
                </section>
              )}

              {editable ? (
                <ContactSection partnerId={id} contacts={contacts} />
              ) : (
                <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
                  <h2 className="text-section font-semibold">{t('contacts')} ({contacts.length})</h2>
                  <div className="mt-3 space-y-2">
                    {contacts.map((c) => (
                      <p key={c.id} className="text-body">
                        <span className="font-medium">{c.name}</span>
                        <span className="text-steel-500">
                          {' '}
                          · <ContactLine position={c.position} email={c.email} phone={c.phone} />
                        </span>
                      </p>
                    ))}
                    {contacts.length === 0 && <p className="text-metadata text-steel-500">—</p>}
                  </div>
                </section>
              )}

              <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
                <h2 className="text-section font-semibold">
                  {tn('orders')} ({orders.length})
                </h2>
                <div className="mt-3 space-y-2">
                  {orders.map((o) => (
                    <Link
                      key={o.id}
                      href={`/${locale}/orders/${o.id}`}
                      className="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-steel-200 px-3 py-2 hover:bg-panel"
                    >
                      <span className="text-body">
                        <span className="font-mono font-medium">{o.number}</span>{' '}
                        <span className="font-medium">{o.title}</span>{' '}
                        <span className="text-steel-500">· {o.stage_label}</span>
                      </span>
                      <Money minor={o.total_minor} currency={o.currency} />
                    </Link>
                  ))}
                  {orders.length === 0 && (
                    <p className="text-metadata text-steel-500">{t('noOrders')}</p>
                  )}
                </div>
              </section>

              {partner.minicrm_id != null && <RawImportPanel entity="partner" id={id} />}

              {!editing && (
                <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
                  <h2 className="text-section font-semibold">{tt('forRecord')}</h2>
                  <div className="mt-3">
                    <TaskList entity="partner" id={id} />
                  </div>
                </section>
              )}

              {!editing && (
                <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
                  <h2 className="text-section font-semibold">{tl('title')}</h2>
                  <div className="mt-3">
                    <Timeline entity="partner" id={id} />
                  </div>
                </section>
              )}

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
