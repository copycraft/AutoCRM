'use client';

import Link from 'next/link';
import { useEffect, useState } from 'react';
import { useLocale, useTranslations } from 'next-intl';
import { skipToken, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { DetailSkeleton } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { StageHistoryList } from '@/components/ui/StageRail';
import { LeadForm, leadPatchBody, type LeadFormValues } from '@/components/forms/LeadForm';
import { TaskList } from '@/components/tasks/TaskList';
import { LeadStageDialog } from '@/components/forms/LeadStageDialog';
import { LeadConvertDialog } from '@/components/forms/LeadConvertDialog';
import { QuotationDialog } from '@/components/email/QuotationDialog';
import { stageTone } from '@/lib/utils/stages';
import { configApi, leadsApi, partnersApi, usersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { canAdmin, canEditLeads, useAuth } from '@/lib/auth/context';
import { daysSince } from '@/lib/utils/format';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { EmailValue, PhoneValue } from '@/components/ui/ContactLinks';
import { Breadcrumbs, BackToList } from '@/components/ui/Breadcrumbs';
import { CopyButton, CopyLinkButton } from '@/components/ui/CopyButton';
import { useRecentRecords } from '@/hooks/useRecent';
import { RawImportPanel } from '@/components/migration/RawImportPanel';
import { Money } from '@/components/ui/Money';

/** A quote whose validity has passed. Plain YYYY-MM-DD compared against the Budapest
 * calendar date — a UTC date would flip the badge around midnight for "today". */
function expired(validUntil: string): boolean {
  const budapest = new Intl.DateTimeFormat('en-CA', { timeZone: 'Europe/Budapest' }).format(new Date());
  return validUntil < budapest;
}

export default function LeadDetailPage({ params }: { params: { id: string } }) {
  const id = Number(params.id);
  const t = useTranslations('leads');
  const tc = useTranslations('common');
  const tn = useTranslations('navigation');
  const tt = useTranslations('tasks');
  const locale = useLocale();
  const { user } = useAuth();
  const qc = useQueryClient();
  const { push: pushRecent } = useRecentRecords(6);
  const [editing, setEditing] = useState(false);
  const [stageOpen, setStageOpen] = useState(false);
  const [convertOpen, setConvertOpen] = useState(false);
  const [quotationOpen, setQuotationOpen] = useState(false);
  const editable = canEditLeads(user);

  const detail = useQuery({ queryKey: qk.lead(id), queryFn: () => leadsApi.get(id) });
  useEffect(() => {
    if (detail.data) {
      pushRecent({
        href: `/${locale}/leads/${id}`,
        title: detail.data.lead.title,
        sub: detail.data.lead.contact_name ?? undefined,
      });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [detail.data?.lead.id]);
  const stagesQuery = useQuery({
    queryKey: qk.stages('lead'),
    queryFn: () => configApi.stages('lead'),
  });
  const partnerId = detail.data?.lead.partner_id ?? null;
  const assignedTo = detail.data?.lead.assigned_to ?? null;
  const partnerQuery = useQuery({
    queryKey: partnerId !== null ? qk.partner(partnerId) : ['partner', 'none'],
    queryFn: partnerId === null ? skipToken : () => partnersApi.get(partnerId),
  });
  // GET /users is admin-only; non-admins see the raw id (backend gap).
  const usersQuery = useQuery({
    queryKey: qk.users,
    queryFn: () => usersApi.list(),
    enabled: canAdmin(user) && assignedTo !== null,
    retry: false,
  });
  const assigneeName =
    assignedTo !== null
      ? (usersQuery.data?.items.find((u) => u.id === assignedTo)?.display_name ?? `#${assignedTo}`)
      : '—';

  const patch = useMutation({
    mutationFn: (body: Record<string, unknown>) => leadsApi.patch(id, body),
    onSuccess: () => {
      setEditing(false);
      void qc.invalidateQueries({ queryKey: qk.lead(id) });
      void qc.invalidateQueries({ queryKey: ['leads'] });
    },
  });

  return (
    <AppShell>
      {detail.isLoading ? (
        <DetailSkeleton />
      ) : detail.isError || !detail.data ? (
        <>
          <PageHeader size="record" title={t('leadDetails')} />
          <ErrorState error={detail.error} onRetry={() => void detail.refetch()} />
        </>
      ) : (
        (() => {
          const { lead, stage, history, orders, documents } = detail.data;
          const defs = stagesQuery.data?.items ?? [];
          const currentDef = defs.find((d) => d.key === stage?.stage_key);
          // V2.7: one enquiry for three vans is three orders; converting again is allowed
          // and every order keeps its origin.
          const converted = orders.length > 0;
          return (
            <>
              <BackToList listKey="leads" fallbackHref={`/${locale}/leads`} />
              <div className="flex items-center justify-between gap-3">
                <Breadcrumbs
                  items={[
                    { href: `/${locale}/leads`, label: tn('leads') },
                    { label: lead.title },
                  ]}
                />
                <CopyLinkButton />
              </div>
              <PageHeader size="record"
                title={
                  <span className="inline-flex items-center gap-2">
                    <span>{lead.title}</span>
                    <CopyButton value={lead.title} label={t('title')} />
                    {lead.contact_email && <CopyButton value={lead.contact_email} label={t('contactEmail')} />}
                    {lead.contact_phone && <CopyButton value={lead.contact_phone} label={t('contactPhone')} />}
                  </span>
                }
                subtitle={`#${lead.id} · ${t('age')}: ${tc('ageDays', { days: daysSince(lead.created_at) })}`}
                actions={editable && (
                  <>
                    {!editing && (
                      <button className="btn-secondary btn-sm" onClick={() => setEditing(true)}>
                        {tc('edit')}
                      </button>
                    )}
                    {!editing && (
                      <Link className="btn-secondary btn-sm" href={`/${locale}/leads/new?clone=${lead.id}`}>
                        {t('clone')}
                      </Link>
                    )}
                    {!converted && (
                      <>
                        <button className="btn-secondary btn-sm" onClick={() => setStageOpen(true)}>
                          {t('stageChange')}
                        </button>
                        <button className="btn-primary btn-sm" onClick={() => setConvertOpen(true)}>
                          {t('convert')}
                        </button>
                      </>
                    )}
                  </>
                )}
              />

              <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
                <div className="flex flex-wrap items-center gap-3">
                  <span className="text-metadata text-steel-500">{t('currentStage')}:</span>
                  <StatusBadge tone={stageTone(currentDef)}>
                    {currentDef?.label_hu ?? stage?.stage_key ?? '—'}
                  </StatusBadge>
                  {stage && (
                    <span className="text-metadata text-steel-500">
                      {t('since')} <DateDisplay withTime value={stage.entered_at} />
                    </span>
                  )}
                  {orders.map((o) => (
                    <Link
                      key={o.id}
                      href={`/${locale}/orders/${o.id}`}
                      className="text-body text-steel-900 underline"
                    >
                      {t('convertedOrder')}: <span className="font-mono">{o.number}</span>
                    </Link>
                  ))}
                </div>
              </section>

              {editing ? (
                <LeadForm
                  initial={lead}
                  initialPartner={
                    lead.partner_id
                      ? {
                          id: lead.partner_id,
                          name: partnerQuery.data?.partner.name ?? `#${lead.partner_id}`,
                        }
                      : null
                  }
                  submitLabel={tc('save')}
                  onSubmit={(v: LeadFormValues) =>
                    patch.mutateAsync(leadPatchBody(lead, v, user?.id)).then(() => undefined)
                  }
                />
              ) : (
                <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
                  <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
                    <Info label={t('partner')} value={partnerQuery.data?.partner.name ?? (lead.partner_id ? `#${lead.partner_id}` : '—')} />
                    <Info label={t('source')} value={lead.source ?? '—'} />
                    <Info label={t('assignedTo')} value={assigneeName} />
                    <Info label={t('contactName')} value={lead.contact_name ?? '—'} />
                    <Info label={t('contactEmail')} value={<EmailValue value={lead.contact_email} />} />
                    <Info label={t('contactPhone')} value={<PhoneValue value={lead.contact_phone} mono />} />
                    <Info label={t('description')} value={lead.description ?? '—'} />
                    <Info label={t('createdAt')} value={<DateDisplay value={lead.created_at} />} />
                  </div>

                  {/* V2.3: the quotation — the six weeks between "we sent them a price"
                      and "they said yes" were invisible before this. */}
                  <div className="mt-5 border-t border-steel-200 pt-5">
                    <div className="flex flex-wrap items-center justify-between gap-3">
                      <h2 className="text-section font-semibold">{t('quoteSection')}</h2>
                      {editable && (
                        <button className="btn-primary btn-sm" onClick={() => setQuotationOpen(true)}>
                          {t('sendQuotation')}
                        </button>
                      )}
                    </div>
                    <div className="mt-3 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
                      <Info
                        label={t('quotedValue')}
                        value={
                          lead.quoted_value_minor != null && lead.currency ? (
                            <Money minor={lead.quoted_value_minor} currency={lead.currency} />
                          ) : (
                            '—'
                          )
                        }
                        mono
                      />
                      <Info
                        label={t('quoteValidUntil')}
                        value={
                          lead.quote_valid_until ? (
                            <span className="flex flex-wrap items-center gap-2">
                              <DateDisplay value={lead.quote_valid_until} />
                              {expired(lead.quote_valid_until) && (
                                <StatusBadge tone="signal">{t('quoteExpired')}</StatusBadge>
                              )}
                            </span>
                          ) : (
                            '—'
                          )
                        }
                      />
                    </div>
                  </div>

                  {/* V2.4: documents can hang off the lead, so the quotation itself has
                      somewhere to live and can be attached to an email. */}
                  <div className="mt-5 border-t border-steel-200 pt-5">
                    <h2 className="text-section font-semibold">
                      {t('documents')} ({documents.length})
                    </h2>
                    <div className="mt-3 space-y-2">
                      {documents.length === 0 && (
                        <p className="text-metadata text-steel-500">{t('noDocuments')}</p>
                      )}
                      {documents.map((d) => (
                        <p key={d.id} className="text-body">
                          <span className="font-medium">{d.filename}</span>{' '}
                          <span className="text-metadata text-steel-500">
                            · <DateDisplay value={d.uploaded_at} />
                          </span>
                        </p>
                      ))}
                    </div>
                  </div>
                </section>
              )}

              <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
                <h2 className="text-section font-semibold">{t('history')}</h2>
                <div className="mt-3">
                  <StageHistoryList history={history} />
                </div>
              </section>

              {lead.minicrm_id != null && <RawImportPanel entity="lead" id={id} />}

              {!editing && (
                <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
                  <h2 className="text-section font-semibold">{tt('forRecord')}</h2>
                  <div className="mt-3">
                    <TaskList entity="lead" id={id} />
                  </div>
                </section>
              )}

              {stageOpen && (
                <LeadStageDialog leadId={id} detail={detail.data} onClose={() => setStageOpen(false)} />
              )}
              {convertOpen && (
                <LeadConvertDialog leadId={id} detail={detail.data} onClose={() => setConvertOpen(false)} />
              )}
              {quotationOpen && (
                <QuotationDialog leadId={id} detail={detail.data} onClose={() => setQuotationOpen(false)} />
              )}
            </>
          );
        })()
      )}
    </AppShell>
  );
}

function Info({ label, value, mono }: { label: string; value: React.ReactNode; mono?: boolean }) {
  return (
    <div className="flex flex-col gap-0.5">
      <dt className="text-metadata font-medium text-steel-500">{label}</dt>
      <dd className={`text-body ${mono ? 'font-mono' : ''}`}>{value}</dd>
    </div>
  );
}


