'use client';

import { useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { useRouter } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { skipToken, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { leadsApi, partnersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { missingQuestions, visibleQuestions } from '@/lib/questionnaire';
import { QuestionField, QuestionnaireFields } from '@/components/spec/QuestionnaireFields';
import { useQuestionnaire } from '@/components/spec/QuestionnaireView';
import { ProjectTypeSelect } from '@/components/spec/LeadSpecSection';
import type { Answers, Currency, LeadDetail } from '@/lib/api/types';

function toCurrency(value: string): Currency {
  return value === 'EUR' ? 'EUR' : 'HUF';
}

/**
 * Lead → order. The build questionnaire has to be complete first: whatever required answer
 * the lead is still missing is asked at the top, the rest of the form and the whole
 * questionnaire below. An answer that opens a section ("Hűteni kell?" → igen) brings that
 * section's required questions up to the top as well, and they stay there once answered so
 * nothing jumps away while being typed. The server checks the same and answers
 * `422 spec_incomplete` if the two ever disagree.
 */
export function LeadConvertDialog({
  leadId,
  detail,
  onClose,
}: {
  leadId: number;
  detail: LeadDetail;
  onClose: () => void;
}) {
  const t = useTranslations('leads');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const tof = useTranslations('orders');
  const ts = useTranslations('spec');
  const locale = useLocale();
  const router = useRouter();
  const qc = useQueryClient();
  const lead = detail.lead;
  const questionnaire = useQuestionnaire();

  const [title, setTitle] = useState(lead.title);
  // Explicit user choice; null means "follow the partner default".
  const [currencyOverride, setCurrencyOverride] = useState<Currency | null>(null);
  const [projectTypeId, setProjectTypeId] = useState<number | null>(detail.spec.project_type_id ?? null);
  const [description, setDescription] = useState(lead.description ?? '');
  const [answers, setAnswers] = useState<Answers>(detail.spec.answers);
  const [error, setError] = useState<string | null>(null);
  // The questions pinned to the top: everything that has been missing since the dialog
  // opened, in questionnaire order. Grows as answers open sections; never shrinks.
  const [pinned, setPinned] = useState<ReadonlySet<string>>(new Set());

  const partnerId = lead.partner_id ?? undefined;
  const partnerQuery = useQuery({
    queryKey: qk.partner(partnerId ?? 0),
    queryFn: partnerId === undefined ? skipToken : () => partnersApi.get(partnerId),
  });

  // Prefilled from the lead's partner default, but always changeable —
  // an EUR job for a HUF-default partner must be possible.
  const partnerDefault: Currency = partnerQuery.data?.partner.default_currency ?? 'HUF';
  const effectiveCurrency = currencyOverride ?? partnerDefault;

  const missing = missingQuestions(questionnaire.data, projectTypeId, answers);
  const missingKeys = missing.map((q) => q.key);
  // Pinning happens during render (not in an effect) so a newly missing question is at the
  // top in the same frame it appears.
  if (missingKeys.some((k) => !pinned.has(k))) {
    setPinned(new Set([...pinned, ...missingKeys]));
  }
  const asked = visibleQuestions(questionnaire.data, projectTypeId, answers);
  const top = asked.filter((q) => pinned.has(q.key) || missingKeys.includes(q.key));
  const topKeys = new Set(top.map((q) => q.key));
  const highlight = new Set(missingKeys);

  const setAnswer = (key: string, value: unknown) =>
    setAnswers((prev) => {
      const next = { ...prev };
      if (value === null || value === undefined) delete next[key];
      else next[key] = value;
      return next;
    });

  const convert = useMutation({
    mutationFn: () =>
      leadsApi.convert(leadId, {
        title: title.trim() || undefined,
        partner_id: partnerId,
        currency: effectiveCurrency,
        project_type_id: projectTypeId ?? undefined,
        description: description.trim() || undefined,
        // The whole set: laid over the lead's own answers, which the server then stores
        // on both the lead and the new order.
        spec_answers: answers,
        items: [],
      }),
    onSuccess: (order) => {
      void qc.invalidateQueries({ queryKey: qk.lead(leadId) });
      void qc.invalidateQueries({ queryKey: ['leads'] });
      void qc.invalidateQueries({ queryKey: ['orders'] });
      router.push(`/${locale}/orders/${order.id}`);
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  const ready = !questionnaire.isLoading && missing.length === 0;
  const canConvert = title.trim() !== '' && partnerId !== undefined && ready && !convert.isPending;

  return (
    <Dialog.Root
      open
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-steel-900/40" />
        <Dialog.Content className="card fixed left-1/2 top-1/2 z-50 max-h-[90vh] w-[94vw] max-w-3xl -translate-x-1/2 -translate-y-1/2 overflow-y-auto">
          <div className="card-header">
            <Dialog.Title className="text-section font-semibold">{t('convertTitle')}</Dialog.Title>
            <Dialog.Description className="text-metadata text-steel-500">{t('wonOnlyViaConvert')}</Dialog.Description>
          </div>
          <div className="card-content space-y-5">
            {top.length > 0 && (
              <section
                className="rounded-lg border border-signal/40 bg-signal/5 p-4"
                aria-labelledby="lc-missing-title"
                data-testid="convert-missing"
              >
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <h3 id="lc-missing-title" className="text-body font-semibold text-steel-900">
                    {ts('missingTitle')}
                  </h3>
                  {missing.length > 0 ? (
                    <span className="badge-signal">{ts('stillMissing', { count: missing.length })}</span>
                  ) : (
                    <span className="badge bg-done/10 text-done">{ts('complete')}</span>
                  )}
                </div>
                <p className="mt-1 text-metadata text-steel-500">{ts('missingIntro')}</p>
                <div className="mt-3 grid grid-cols-1 gap-4 md:grid-cols-2">
                  {top.map((q) => (
                    <QuestionField
                      key={q.key}
                      question={q}
                      value={answers[q.key]}
                      onChange={(v) => setAnswer(q.key, v)}
                      highlight={highlight.has(q.key)}
                      idPrefix="lcm"
                    />
                  ))}
                </div>
              </section>
            )}

            <div className="space-y-4">
              <div>
                <label className="label" htmlFor="lc-title">{t('title')} *</label>
                <input id="lc-title" className="input" value={title} onChange={(e) => setTitle(e.target.value)} />
              </div>
              <div>
                <span className="label">{t('convertPartner')}</span>
                <p className="rounded-lg border border-steel-200 bg-panel px-3 py-2 text-body">
                  {partnerQuery.data?.partner.name ?? '—'}
                </p>
                {!partnerId && (
                  <p className="mt-1 text-metadata text-steel-900">{t('noPartnerForConvert')}</p>
                )}
              </div>
              <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                <div>
                  <label className="label" htmlFor="lc-currency">{t('convertCurrency')}</label>
                  <select
                    id="lc-currency"
                    className="input"
                    value={effectiveCurrency}
                    onChange={(e) => setCurrencyOverride(toCurrency(e.target.value))}
                  >
                    <option value="HUF">HUF</option>
                    <option value="EUR">EUR</option>
                  </select>
                  {partnerQuery.data && currencyOverride === null && (
                    <p className="mt-1 text-metadata text-steel-500">{t('partnerCurrencyHint')}</p>
                  )}
                </div>
                <div>
                  <label className="label" htmlFor="lc-pt">{tof('projectType')}</label>
                  <ProjectTypeSelect id="lc-pt" value={projectTypeId} onChange={setProjectTypeId} />
                  {projectTypeId === null && (
                    <p className="mt-1 text-metadata text-steel-500">{ts('chooseType')}</p>
                  )}
                </div>
              </div>
              <div>
                <label className="label" htmlFor="lc-desc">{t('description')}</label>
                <textarea id="lc-desc" rows={3} className="input" value={description} onChange={(e) => setDescription(e.target.value)} />
              </div>
            </div>

            <section className="border-t border-steel-200 pt-4" aria-labelledby="lc-rest-title">
              <h3 id="lc-rest-title" className="mb-3 text-body font-semibold text-steel-900">
                {ts('restTitle')}
              </h3>
              <QuestionnaireFields
                questionnaire={questionnaire.data}
                projectTypeId={projectTypeId}
                answers={answers}
                onChange={setAnswers}
                exclude={topKeys}
                idPrefix="lc"
              />
            </section>

            {error && (
              <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">{error}</p>
            )}
          </div>
          <div className="card-footer justify-end">
            {missing.length > 0 && (
              <span className="mr-auto text-metadata text-signal">{ts('stillMissing', { count: missing.length })}</span>
            )}
            <button className="btn-ghost" onClick={onClose}>{tc('cancel')}</button>
            <button className="btn-primary" disabled={!canConvert} onClick={() => convert.mutate()}>
              {convert.isPending ? t('converting') : t('convert')}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
