'use client';

import Link from 'next/link';
import { useState } from 'react';
import { useLocale, useTranslations } from 'next-intl';
import { skipToken, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { DetailSkeleton } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { StageHistoryList } from '@/components/ui/StageRail';
import { LeadForm, leadPatchBody, type LeadFormValues } from '@/components/forms/LeadForm';
import { LeadStageDialog } from '@/components/forms/LeadStageDialog';
import { LeadConvertDialog } from '@/components/forms/LeadConvertDialog';
import { stageTone } from '@/lib/utils/stages';
import { configApi, leadsApi, partnersApi, usersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { canAdmin, canEditLeads, useAuth } from '@/lib/auth/context';
import { daysSince } from '@/lib/utils/format';
import { DateDisplay } from '@/components/ui/DateDisplay';

export default function LeadDetailPage({ params }: { params: { id: string } }) {
  const id = Number(params.id);
  const t = useTranslations('leads');
  const tc = useTranslations('common');
  const locale = useLocale();
  const { user } = useAuth();
  const qc = useQueryClient();
  const [editing, setEditing] = useState(false);
  const [stageOpen, setStageOpen] = useState(false);
  const [convertOpen, setConvertOpen] = useState(false);
  const editable = canEditLeads(user);

  const detail = useQuery({ queryKey: qk.lead(id), queryFn: () => leadsApi.get(id) });
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
          <PageHeader title={t('leadDetails')} />
          <ErrorState error={detail.error} onRetry={() => void detail.refetch()} />
        </>
      ) : (
        (() => {
          const { lead, stage, history, order } = detail.data;
          const defs = stagesQuery.data?.items ?? [];
          const currentDef = defs.find((d) => d.key === stage?.stage_key);
          const converted = order !== null;
          return (
            <>
              <PageHeader
                title={lead.title}
                subtitle={`#${lead.id} · ${t('age')}: ${tc('ageDays', { days: daysSince(lead.created_at) })}`}
                actions={editable && (
                  <>
                    {!editing && (
                      <button className="btn-secondary btn-sm" onClick={() => setEditing(true)}>
                        {tc('edit')}
                      </button>
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

              <section className="card">
                <div className="card-content flex flex-wrap items-center gap-3">
                  <span className="text-sm text-steel-500">{t('currentStage')}:</span>
                  <StatusBadge tone={stageTone(currentDef)}>
                    {currentDef?.label_hu ?? stage?.stage_key ?? '—'}
                  </StatusBadge>
                  {stage && (
                    <span className="text-metadata text-steel-500">
                      {t('since')} <DateDisplay withTime value={stage.entered_at} />
                    </span>
                  )}
                  {converted && order && (
                    <Link href={`/${locale}/orders/${order.id}`} className="text-sm text-steel-900 underline">
                      {t('convertedOrder')}: <span className="font-mono">{order.number}</span>
                    </Link>
                  )}
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
                <section className="card">
                  <div className="card-content grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
                    <Info label={t('partner')} value={partnerQuery.data?.partner.name ?? (lead.partner_id ? `#${lead.partner_id}` : '—')} />
                    <Info label={t('source')} value={lead.source ?? '—'} />
                    <Info label={t('assignedTo')} value={assigneeName} />
                    <Info label={t('contactName')} value={lead.contact_name ?? '—'} />
                    <Info label={t('contactEmail')} value={lead.contact_email ?? '—'} />
                    <Info label={t('contactPhone')} value={lead.contact_phone ?? '—'} />
                    <Info label={t('description')} value={lead.description ?? '—'} />
                    <Info label={t('createdAt')} value={<DateDisplay value={lead.created_at} />} />
                  </div>
                </section>
              )}

              <section className="card">
                <div className="card-header">
                  <h2 className="text-section font-semibold">{t('history')}</h2>
                </div>
                <div className="card-content">
                  <StageHistoryList history={history} />
                </div>
              </section>

              {stageOpen && (
                <LeadStageDialog leadId={id} detail={detail.data} onClose={() => setStageOpen(false)} />
              )}
              {convertOpen && (
                <LeadConvertDialog leadId={id} detail={detail.data} onClose={() => setConvertOpen(false)} />
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
      <dd className={`text-sm ${mono ? 'font-mono' : ''}`}>{value}</dd>
    </div>
  );
}


