'use client';

// Quotation letter for a lead: hero band, the lead's quotation PDF attached, sent by the
// staff member as themselves. Empty subject/hero/body means "use the backend defaults"
// (built from the lead's own figures), so the common case is ticking the PDF and sending.

import { useMemo, useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { useRouter } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { emailApi, followupsApi, leadsApi } from '@/lib/api/endpoints';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { errorMessage } from '@/lib/api/errors';
import { useToast } from '@/components/ui/Toasts';
import type { LeadDetail } from '@/lib/api/types';

export function QuotationDialog({
  leadId,
  detail,
  onClose,
}: {
  leadId: number;
  detail: LeadDetail;
  onClose: () => void;
}) {
  const t = useTranslations('leads');
  const tf = useTranslations('followups');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const locale = useLocale();
  const router = useRouter();
  const qc = useQueryClient();
  const toast = useToast();
  const { lead, documents } = detail;

  const [subject, setSubject] = useState(`Árajánlatunk: ${detail.lead.title}`);
  const [hero, setHero] = useState('Megjött az Autotherm árajánlatod!');
  const [body, setBody] = useState('');
  const [markdown, setMarkdown] = useState(false);
  const [selected, setSelected] = useState<number[]>(() =>
    documents.filter((d) => d.filename.toLowerCase().endsWith('.pdf')).map((d) => d.id),
  );
  const [error, setError] = useState<string | null>(null);
  // Follow-up letters after this quotation: the active default steps, ticked; null until
  // the steps load, so an untouched dialog sends "the defaults".
  const steps = useQuery({ queryKey: ['followup-steps'], queryFn: () => followupsApi.steps() });
  const [followups, setFollowups] = useState<number[] | null>(null);
  const activeSteps = (steps.data?.items ?? []).filter((s) => s.is_active);
  const chosenSteps = followups ?? activeSteps.map((s) => s.id);

  const toggle = (id: number) =>
    setSelected((prev) => (prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id]));

  // Live preview through the same endpoint the send will store: what you see is what the
  // customer gets, hero band included. Empty fields preview as the backend defaults.
  // (The hero default is the backend's; keep in sync with send_quotation.)
  const draftKey = JSON.stringify({
    lead_id: leadId,
    to: lead.contact_email ?? 'preview@invalid',
    subject: subject || null,
    hero: hero.trim() || 'Megjött az Autotherm árajánlatod!',
    body: body || null,
    body_markdown: markdown,
  });
  const debouncedKey = useDebouncedValue(draftKey, 600);
  const preview = useQuery({
    queryKey: ['quotation-preview', leadId, debouncedKey],
    queryFn: () => emailApi.preview(JSON.parse(debouncedKey)),
    enabled: (lead.contact_email ?? '') !== '',
    retry: false,
  });

  const send = useMutation({
    mutationFn: () =>
      leadsApi.quotation(leadId, {
        subject: subject.trim() || null,
        hero: hero.trim() || null,
        body: body.trim() || null,
        body_markdown: markdown,
        attachment_document_ids: selected,
        followup_step_ids: followups ?? undefined,
      }),
    onSuccess: (sent) => {
      void qc.invalidateQueries({ queryKey: ['leads'] });
      toast.success(t('quotationQueued'));
      onClose();
      router.push(`/${locale}/emails/${sent.email_id}`);
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  const recipient = lead.contact_email ?? t('noEmailForQuotation');

  return (
    <Dialog.Root
      open
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-steel-900/40" />
        <Dialog.Content className="card fixed left-1/2 top-1/2 z-50 max-h-[85vh] w-[90vw] max-w-lg -translate-x-1/2 -translate-y-1/2 overflow-y-auto">
          <div className="card-header">
            <Dialog.Title className="text-section font-semibold">{t('quotationTitle')}</Dialog.Title>
            <Dialog.Description className="text-metadata text-steel-500">
              {t('quotationTo', { email: recipient })}
            </Dialog.Description>
          </div>
          <div className="card-content space-y-4">
            <div>
              <label className="label" htmlFor="q-subject">{t('quotationSubject')}</label>
              <input
                id="q-subject"
                className="input"
                value={subject}
                onChange={(e) => setSubject(e.target.value)}
                placeholder={t('quotationSubjectHint', { title: lead.title })}
              />
            </div>
            <div>
              <label className="label" htmlFor="q-hero">{t('quotationHero')}</label>
              <input
                id="q-hero"
                className="input"
                value={hero}
                onChange={(e) => setHero(e.target.value)}
                placeholder={t('quotationHeroHint')}
              />
            </div>
            <div>
              <div className="flex items-center justify-between gap-3">
                <label className="label" htmlFor="q-body">{t('quotationBody')}</label>
                <label className="inline-flex items-center gap-2 text-metadata text-steel-500">
                  <input type="checkbox" checked={markdown} onChange={(e) => setMarkdown(e.target.checked)} />
                  {t('quotationMarkdown')}
                </label>
              </div>
              <textarea
                id="q-body"
                className="input mt-1 font-mono"
                rows={6}
                value={body}
                onChange={(e) => setBody(e.target.value)}
                placeholder={t('quotationBodyHint')}
              />
            </div>
            <div>
              <span className="label">{t('quotationAttachments')}</span>
              {documents.length === 0 ? (
                <p className="mt-1 text-metadata text-steel-500">{t('noDocuments')}</p>
              ) : (
                <ul className="mt-1 space-y-1">
                  {documents.map((d) => (
                    <li key={d.id}>
                      <label className="inline-flex items-center gap-2 text-body">
                        <input type="checkbox" checked={selected.includes(d.id)} onChange={() => toggle(d.id)} />
                        <span className="font-medium">{d.filename}</span>
                      </label>
                    </li>
                  ))}
                </ul>
              )}
            </div>
            {activeSteps.length > 0 && (
              <div>
                <span className="label">{tf('quotationFollowups')}</span>
                <p className="text-metadata text-steel-500">{tf('quotationFollowupsHint')}</p>
                <ul className="mt-1 flex flex-wrap gap-3">
                  {activeSteps.map((s) => (
                    <li key={s.id}>
                      <label className="inline-flex items-center gap-2 text-body" title={s.template_name}>
                        <input
                          type="checkbox"
                          checked={chosenSteps.includes(s.id)}
                          onChange={() =>
                            setFollowups(
                              chosenSteps.includes(s.id) ? chosenSteps.filter((x) => x !== s.id) : [...chosenSteps, s.id],
                            )
                          }
                        />
                        {s.label}
                      </label>
                    </li>
                  ))}
                </ul>
              </div>
            )}
            {error && (
              <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">
                {error}
              </p>
            )}
            <div>
              <span className="label">{t('quotationPreview')}</span>
              <div className="mt-1">
                {preview.data ? (
                  <>
                    <p className="mb-1 text-body font-medium">{preview.data.subject}</p>
                    <iframe
                      title={t('quotationPreview')}
                      sandbox=""
                      srcDoc={preview.data.body_html}
                      className="h-64 w-full rounded-lg border border-steel-200 bg-white"
                    />
                  </>
                ) : (
                  <p className="text-metadata text-steel-500">{t('quotationPreviewHint')}</p>
                )}
              </div>
            </div>
            <div className="flex justify-end gap-2">
              <button type="button" className="btn-secondary btn-sm" onClick={onClose}>
                {tc('cancel')}
              </button>
              <button
                type="button"
                className="btn-primary btn-sm"
                disabled={send.isPending}
                onClick={() => {
                  setError(null);
                  send.mutate();
                }}
              >
                {send.isPending ? '…' : t('quotationSend')}
              </button>
            </div>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
