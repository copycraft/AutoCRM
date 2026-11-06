'use client';

// Recording a quotation. The sales manager writes the quotation outside the CRM and sends
// it from their own mailbox; here they file the PDF on the lead, note the price and how
// long it holds, and the automatic follow-up letters start counting from the day it went
// out. Nothing is emailed from here.

import { useRef, useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { format } from 'date-fns';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { followupsApi, leadsApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { minorToMajorString } from '@/lib/utils/format';
import { sha256Hex, uploadFile } from '@/lib/upload';
import { useLookups } from '@/hooks/useLookups';
import { useToast } from '@/components/ui/Toasts';
import { DateQuickPicks } from '@/components/forms/DateQuickPicks';
import { toMinor } from '@/components/forms/LeadForm';
import type { Currency, Document, LeadDetail } from '@/lib/api/types';

/** The file to preselect: the newest one already filed as a quotation, else the newest PDF. */
function initialDocument(documents: Document[]): number | null {
  const newest = [...documents].sort((a, b) => b.uploaded_at.localeCompare(a.uploaded_at));
  return (
    newest.find((d) => d.kind === 'quote')?.id ??
    newest.find((d) => d.filename.toLowerCase().endsWith('.pdf'))?.id ??
    null
  );
}

export function QuoteUploadDialog({
  leadId,
  detail,
  hasEmail,
  onClose,
}: {
  leadId: number;
  detail: LeadDetail;
  /** Whether a follow-up letter has an address to go to (the lead's or the partner's). */
  hasEmail: boolean;
  onClose: () => void;
}) {
  const t = useTranslations('leads');
  const tf = useTranslations('followups');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const toast = useToast();
  const { data: lookups } = useLookups();
  const currencies = lookups?.currencies ?? [];
  const { lead } = detail;
  const today = format(new Date(), 'yyyy-MM-dd');

  const [documents, setDocuments] = useState<Document[]>(detail.documents);
  const [documentId, setDocumentId] = useState<number | null>(() => initialDocument(detail.documents));
  const [value, setValue] = useState(() =>
    lead.quoted_value_minor == null ? '' : minorToMajorString(lead.quoted_value_minor),
  );
  const [currency, setCurrency] = useState<string>(lead.currency ?? 'HUF');
  const [validUntil, setValidUntil] = useState(lead.quote_valid_until ?? '');
  const [sentOn, setSentOn] = useState(today);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);

  // The active default steps, ticked; null until touched, so an untouched dialog sends
  // "the defaults". Without an address none is ticked: they could only be skipped.
  const steps = useQuery({ queryKey: ['followup-steps', 'quote'], queryFn: () => followupsApi.steps('quote') });
  const [followups, setFollowups] = useState<number[] | null>(hasEmail ? null : []);
  const activeSteps = (steps.data?.items ?? []).filter((s) => s.is_active);
  const chosenSteps = followups ?? activeSteps.map((s) => s.id);

  const upload = async (file: File) => {
    setUploading(true);
    setError(null);
    try {
      const r = await uploadFile({ lead: leadId }, file, { type: 'document', kind: 'quote' });
      if (r.status === 'already_uploaded') {
        // Already on the lead: find it by its hash and pick it.
        const hash = await sha256Hex(file);
        const same = documents.find((d) => d.content_hash === hash);
        if (same) setDocumentId(same.id);
        toast.info(t('quoteFileAlreadyThere'));
      } else if (r.completed.type === 'document') {
        const doc = r.completed.document;
        setDocuments((prev) => [doc, ...prev.filter((d) => d.id !== doc.id)]);
        setDocumentId(doc.id);
      }
      void qc.invalidateQueries({ queryKey: qk.leadDocuments(leadId) });
    } catch (e) {
      setError(errorMessage(e, ter, ter('unknownError')));
    } finally {
      setUploading(false);
      if (fileInput.current) fileInput.current.value = '';
    }
  };

  const record = useMutation({
    mutationFn: () => {
      const minor = toMinor(value);
      return leadsApi.recordQuotation(leadId, {
        document_id: documentId!,
        quoted_value_minor: minor,
        currency: minor == null ? null : (currency as Currency),
        quote_valid_until: validUntil || null,
        sent_on: sentOn || null,
        followup_step_ids: followups ?? undefined,
      });
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: qk.lead(leadId) });
      void qc.invalidateQueries({ queryKey: ['leads'] });
      void qc.invalidateQueries({ queryKey: ['followups', leadId] });
      toast.success(chosenSteps.length > 0 ? t('quoteRecorded') : t('quoteRecordedNoFollowups'));
      onClose();
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  const submit = () => {
    setError(null);
    if (documentId == null) {
      setError(t('quoteNeedsFile'));
      return;
    }
    if (chosenSteps.length > 0 && !hasEmail) {
      setError(t('quoteNoEmail'));
      return;
    }
    record.mutate();
  };

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
            <Dialog.Title className="text-section font-semibold">{t('quoteUploadTitle')}</Dialog.Title>
            <Dialog.Description className="text-metadata text-steel-500">{t('quoteUploadIntro')}</Dialog.Description>
          </div>
          <div className="card-content space-y-4">
            <div>
              <span className="label">{t('quoteFile')}</span>
              <div className="mt-1 flex flex-wrap items-center gap-3">
                <button
                  type="button"
                  className="btn-secondary btn-sm"
                  disabled={uploading}
                  onClick={() => fileInput.current?.click()}
                >
                  {uploading ? tc('processing') : t('quoteFilePick')}
                </button>
                <input
                  ref={fileInput}
                  type="file"
                  className="hidden"
                  accept=".pdf,application/pdf,image/*,.doc,.docx,.xls,.xlsx"
                  onChange={(e) => {
                    const file = e.target.files?.[0];
                    if (file) void upload(file);
                  }}
                />
              </div>
              {documents.length > 0 && (
                <>
                  <p className="mt-3 text-metadata text-steel-500">{t('quoteFileExisting')}</p>
                  <ul className="mt-1 space-y-1">
                    {documents.map((d) => (
                      <li key={d.id}>
                        <label className="inline-flex items-center gap-2 text-body">
                          <input
                            type="radio"
                            name="quote-document"
                            checked={documentId === d.id}
                            onChange={() => setDocumentId(d.id)}
                          />
                          <span className="font-medium">{d.filename}</span>
                        </label>
                      </li>
                    ))}
                  </ul>
                </>
              )}
            </div>

            <div className="grid grid-cols-1 gap-3 sm:grid-cols-[1fr_auto]">
              <div>
                <label className="label" htmlFor="qu-value">{t('quotedValue')}</label>
                <input
                  id="qu-value"
                  inputMode="decimal"
                  className="input font-mono"
                  value={value}
                  onChange={(e) => setValue(e.target.value)}
                />
              </div>
              <div>
                <label className="label" htmlFor="qu-currency">{t('quoteCurrency')}</label>
                <select id="qu-currency" className="input" value={currency} onChange={(e) => setCurrency(e.target.value)}>
                  {currencies.map((c) => (
                    <option key={c.key} value={c.key}>
                      {c.key}
                    </option>
                  ))}
                </select>
              </div>
            </div>

            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              <div>
                <label className="label" htmlFor="qu-valid">{t('quoteValidUntil')}</label>
                <input
                  id="qu-valid"
                  type="date"
                  className="input"
                  value={validUntil}
                  onChange={(e) => setValidUntil(e.target.value)}
                />
                <div className="mt-1">
                  <DateQuickPicks onPick={setValidUntil} />
                </div>
              </div>
              <div>
                <label className="label" htmlFor="qu-sent">{t('quoteSentOn')}</label>
                <input
                  id="qu-sent"
                  type="date"
                  className="input"
                  max={today}
                  value={sentOn}
                  onChange={(e) => setSentOn(e.target.value)}
                />
              </div>
            </div>

            {activeSteps.length > 0 && (
              <div>
                <span className="label">{tf('quotationFollowups')}</span>
                <p className="text-metadata text-steel-500">
                  {hasEmail ? tf('quotationFollowupsHint') : t('quoteNoEmail')}
                </p>
                <ul className="mt-1 flex flex-wrap gap-3">
                  {activeSteps.map((s) => (
                    <li key={s.id}>
                      <label className="inline-flex items-center gap-2 text-body" title={s.template_name}>
                        <input
                          type="checkbox"
                          disabled={!hasEmail}
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

            <div className="flex justify-end gap-2">
              <button type="button" className="btn-secondary btn-sm" onClick={onClose}>
                {tc('cancel')}
              </button>
              <button
                type="button"
                className="btn-primary btn-sm"
                disabled={record.isPending || uploading}
                onClick={submit}
              >
                {record.isPending ? '…' : t('quoteSave')}
              </button>
            </div>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
