'use client';

// The default follow-up sequence after a quotation: when each letter goes (1 hét, 2 hét,
// 1 hónap...) and which template it sends. Every quotation schedules the active steps,
// counted from its send; the office can still change them per lead.

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { CalendarClock, Plus } from 'lucide-react';
import { emailApi, followupsApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { cn } from '@/lib/utils/format';
import type { FollowupStep } from '@/lib/api/types';

/** The intervals offered with one click; anything else is typed in days. */
export const PRESETS: { label: string; days: number }[] = [
  { label: '3 nap', days: 3 },
  { label: '1 hét', days: 7 },
  { label: '2 hét', days: 14 },
  { label: '3 hét', days: 21 },
  { label: '1 hónap', days: 30 },
  { label: '2 hónap', days: 60 },
  { label: '3 hónap', days: 90 },
];

export function presetLabel(days: number): string {
  return PRESETS.find((p) => p.days === days)?.label ?? `${days} nap`;
}

/**
 * `kind`: quote — days after a quotation goes out; invoice — days after an unpaid
 * invoice's payment deadline (the automatic payment reminders).
 */
export function FollowupSteps({ editable, kind = 'quote' }: { editable: boolean; kind?: 'quote' | 'invoice' }) {
  const t = useTranslations('followups');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const steps = useQuery({ queryKey: ['followup-steps', kind], queryFn: () => followupsApi.steps(kind) });
  const templates = useQuery({ queryKey: ['email-templates'], queryFn: () => emailApi.templates() });
  const [error, setError] = useState<string | null>(null);
  const [days, setDays] = useState(7);
  const [templateKey, setTemplateKey] = useState(kind === 'invoice' ? 'invoice_overdue_1' : 'quote_followup_1');

  const live = (templates.data?.items ?? []).filter((x) => !x.archived_at);
  const refresh = () => {
    setError(null);
    void qc.invalidateQueries({ queryKey: ['followup-steps'] });
  };
  const onError = (e: unknown) => setError(errorMessage(e, ter, ter('unknownError')));
  const update = useMutation({
    mutationFn: ({ id, body }: { id: number; body: Parameters<typeof followupsApi.updateStep>[1] }) =>
      followupsApi.updateStep(id, body),
    onSuccess: refresh,
    onError,
  });
  const create = useMutation({
    mutationFn: () => followupsApi.createStep({ label: presetLabel(days), delay_days: days, template_key: templateKey, kind }),
    onSuccess: refresh,
    onError,
  });

  return (
    <section className="card" data-testid={`followup-steps-${kind}`}>
      <div className="card-header flex items-center gap-2">
        <CalendarClock className="h-5 w-5 text-steel-500" aria-hidden />
        <h2 className="text-section font-semibold">{t(kind === 'invoice' ? 'reminderStepsTitle' : 'stepsTitle')}</h2>
      </div>
      <div className="card-content space-y-3">
        <p className="text-metadata text-steel-500">{t(kind === 'invoice' ? 'reminderStepsIntro' : 'stepsIntro')}</p>
        {error && <p className="text-body text-signal" role="alert">{error}</p>}
        <ol className="space-y-2">
          {(steps.data?.items ?? []).map((s: FollowupStep) => (
            <li key={s.id} className={cn('flex flex-wrap items-center gap-2 rounded-lg border border-steel-200 p-2', !s.is_active && 'opacity-60')}>
              <select
                className="input h-8 w-32 py-0"
                aria-label={t('interval')}
                value={s.delay_days}
                disabled={!editable}
                onChange={(e) => {
                  const d = Number(e.target.value);
                  update.mutate({ id: s.id, body: { delay_days: d, label: presetLabel(d) } });
                }}
              >
                {[...new Set([...PRESETS.map((p) => p.days), s.delay_days])].sort((a, b) => a - b).map((d) => (
                  <option key={d} value={d}>{presetLabel(d)}</option>
                ))}
              </select>
              <span className="text-metadata text-steel-500">{t(kind === 'invoice' ? 'afterDeadline' : 'afterQuote')}</span>
              <select
                className="input h-8 min-w-0 flex-1 py-0"
                aria-label={t('template')}
                value={s.template_key}
                disabled={!editable}
                onChange={(e) => update.mutate({ id: s.id, body: { template_key: e.target.value } })}
              >
                {live.map((tpl) => (
                  <option key={tpl.key} value={tpl.key}>{tpl.name}</option>
                ))}
              </select>
              <label className="flex items-center gap-1.5 text-body">
                <input
                  type="checkbox"
                  className="rounded border-steel-200 accent-steel-900"
                  checked={s.is_active}
                  disabled={!editable}
                  onChange={(e) => update.mutate({ id: s.id, body: { is_active: e.target.checked } })}
                />
                {t('active')}
              </label>
            </li>
          ))}
        </ol>
        {editable && (
          <div className="flex flex-wrap items-center gap-2 border-t border-steel-200 pt-3">
            <span className="text-body">{t('addStep')}</span>
            <select className="input h-8 w-32 py-0" aria-label={t('interval')} value={days} onChange={(e) => setDays(Number(e.target.value))}>
              {PRESETS.map((p) => (
                <option key={p.days} value={p.days}>{p.label}</option>
              ))}
            </select>
            <select className="input h-8 min-w-0 flex-1 py-0" aria-label={t('template')} value={templateKey} onChange={(e) => setTemplateKey(e.target.value)}>
              {live.map((tpl) => (
                <option key={tpl.key} value={tpl.key}>{tpl.name}</option>
              ))}
            </select>
            <button type="button" className="btn-secondary btn-sm" disabled={create.isPending} onClick={() => create.mutate()}>
              <Plus className="h-4 w-4" aria-hidden />
              {t('add')}
            </button>
          </div>
        )}
      </div>
    </section>
  );
}
