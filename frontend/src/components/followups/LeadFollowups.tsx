'use client';

// A lead's follow-up letters: what is scheduled and when, what went out, what was stopped
// and why. Add one more (1 hét, 2 hét, 1 hónap... from today), stop one, or stop them all.

import { useState } from 'react';
import Link from 'next/link';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { CalendarClock, X } from 'lucide-react';
import { emailApi, followupsApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { StatusBadge, type StatusTone } from '@/components/ui/StatusBadge';
import { PRESETS, presetLabel } from './FollowupSteps';

const TONE: Record<string, StatusTone> = { scheduled: 'cold', sent: 'done', cancelled: 'muted', skipped: 'steel' };

export function LeadFollowups({ leadId, editable, hasEmail }: { leadId: number; editable: boolean; hasEmail: boolean }) {
  const t = useTranslations('followups');
  const ter = useTranslations('errors');
  const locale = useLocale();
  const qc = useQueryClient();
  const list = useQuery({ queryKey: ['followups', leadId], queryFn: () => followupsApi.forLead(leadId) });
  const templates = useQuery({ queryKey: ['email-templates'], queryFn: () => emailApi.templates(), enabled: editable });
  const [days, setDays] = useState(7);
  const [templateKey, setTemplateKey] = useState('quote_followup_1');
  const [error, setError] = useState<string | null>(null);

  const refresh = () => {
    setError(null);
    void qc.invalidateQueries({ queryKey: ['followups', leadId] });
  };
  const onError = (e: unknown) => setError(errorMessage(e, ter, ter('unknownError')));
  const add = useMutation({
    mutationFn: () => followupsApi.schedule(leadId, { delay_days: days, template_key: templateKey, label: presetLabel(days) }),
    onSuccess: refresh,
    onError,
  });
  const cancel = useMutation({ mutationFn: (id: number) => followupsApi.cancel(id), onSuccess: refresh, onError });
  const cancelAll = useMutation({ mutationFn: () => followupsApi.cancelAll(leadId), onSuccess: refresh, onError });

  const rows = list.data?.items ?? [];
  const waiting = rows.filter((f) => f.status === 'scheduled').length;
  const live = (templates.data?.items ?? []).filter((x) => !x.archived_at);

  return (
    <div className="space-y-3" data-testid="lead-followups">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="flex items-center gap-2 text-section font-semibold">
          <CalendarClock className="h-5 w-5 text-steel-500" aria-hidden />
          {t('leadTitle')}
        </h2>
        {editable && waiting > 0 && (
          <button type="button" className="btn-ghost btn-sm" disabled={cancelAll.isPending} onClick={() => cancelAll.mutate()}>
            {t('stopAll', { n: waiting })}
          </button>
        )}
      </div>
      {!hasEmail && <p className="text-metadata text-signal">{t('noEmail')}</p>}
      {error && <p className="text-body text-signal" role="alert">{error}</p>}
      {rows.length === 0 ? (
        <p className="text-metadata text-steel-500">{t('none')}</p>
      ) : (
        <ul className="space-y-1.5">
          {rows.map((f) => (
            <li key={f.id} className="flex flex-wrap items-center gap-2 text-body">
              <StatusBadge tone={TONE[f.status] ?? 'steel'}>{t(`status.${f.status}`)}</StatusBadge>
              <span className="font-medium">{f.label}</span>
              <span className="text-steel-500">· <DateDisplay withTime value={f.due_at} /></span>
              <span className="truncate text-metadata text-steel-500">· {f.template_name}</span>
              {f.email_id && (
                <Link href={`/${locale}/emails/${f.email_id}`} className="text-metadata underline">{t('openEmail')}</Link>
              )}
              {f.note && <span className="text-metadata text-steel-500">({f.note})</span>}
              {editable && f.status === 'scheduled' && (
                <button type="button" className="ml-auto text-steel-500 hover:text-steel-900" aria-label={t('stop')} title={t('stop')} onClick={() => cancel.mutate(f.id)}>
                  <X className="h-4 w-4" aria-hidden />
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
      {editable && (
        <div className="flex flex-wrap items-center gap-2 border-t border-steel-200 pt-3">
          <span className="text-body">{t('addOne')}</span>
          <select className="input h-8 w-32 py-0" aria-label={t('interval')} value={days} onChange={(e) => setDays(Number(e.target.value))}>
            {PRESETS.map((p) => (
              <option key={p.days} value={p.days}>{p.label}</option>
            ))}
          </select>
          <span className="text-metadata text-steel-500">{t('fromToday')}</span>
          <select className="input h-8 min-w-0 flex-1 py-0" aria-label={t('template')} value={templateKey} onChange={(e) => setTemplateKey(e.target.value)}>
            {live.map((tpl) => (
              <option key={tpl.key} value={tpl.key}>{tpl.name}</option>
            ))}
          </select>
          <button type="button" className="btn-secondary btn-sm" disabled={add.isPending} onClick={() => add.mutate()}>
            {t('schedule')}
          </button>
        </div>
      )}
    </div>
  );
}
