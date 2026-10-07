'use client';

// Manual email composer: direct letter or newsletter blast, written once and previewed
// live. The body is plain text or Markdown (never both with a template — the backend
// refuses that mix); the preview endpoint renders exactly what the send will store, so
// what you see in the iframe is what the customer gets.
//
// Attachments come from the record the letter is about, or — for a context-free letter
// like a newsletter — from the recent-documents library. Images can also be embedded:
// pick them, write `![alt](doc:ID)` in Markdown, and they travel inside the letter.

import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery } from '@tanstack/react-query';
import { emailApi, leadsApi, mediaApi, newsletterApi } from '@/lib/api/endpoints';
import { NewsletterChips, NewsletterTagPicker, useNewsletterTags } from '@/components/marketing/NewsletterTags';
import { lookupLabel, useLookups } from '@/hooks/useLookups';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { errorMessage } from '@/lib/api/errors';
import { usePreferences } from '@/hooks/usePreferences';
import { NEWSLETTER_LANGUAGES } from '@/components/tables/BulkBar';
import type { components } from '@/lib/api/schema.gen';

type ComposeRequest = components['schemas']['ComposeRequest'];
type Variant = { language: string; subject: string; hero: string; body: string };
type Audience = 'direct' | 'newsletter';

const PREVIEW_DEBOUNCE_MS = 600;

export function ComposeForm({
  about,
  defaultTo,
  defaultAudience,
  defaultTagIds,
  defaultSubject,
  replyToInboundId,
  quote,
  onSent,
}: {
  about?: { order_id?: number; lead_id?: number; partner_id?: number };
  defaultTo?: string;
  defaultAudience?: Audience;
  defaultTagIds?: number[];
  /** "Re: …" when answering a customer's letter. */
  defaultSubject?: string;
  /** The received letter this answers: the reply threads under it in their mail app. */
  replyToInboundId?: number;
  /** The answered letter's text, quoted under the reply. */
  quote?: string;
  onSent: (emailId: number, newsletterRecipients?: number) => void;
}) {
  const t = useTranslations('emails');
  const ter = useTranslations('errors');
  const [audience, setAudience] = useState<Audience>(defaultAudience ?? 'direct');
  const [tagIds, setTagIds] = useState<number[]>(defaultTagIds ?? []);
  // Empty: the newsletter goes now. A local date-time schedules it.
  const [sendAt, setSendAt] = useState('');
  const [to, setTo] = useState(defaultTo ?? '');
  const [cc, setCc] = useState('');
  const [templateKey, setTemplateKey] = useState('');
  const [theme, setTheme] = useState('');
  const [subject, setSubject] = useState(defaultSubject ?? '');
  const [hero, setHero] = useState('');
  const [body, setBody] = useState('');
  const [markdown, setMarkdown] = useState(false);
  const [selectedDocs, setSelectedDocs] = useState<number[]>([]);
  const [libraryDocs, setLibraryDocs] = useState<number[]>([]);
  const [embeds, setEmbeds] = useState<{ id: number; filename: string }[]>([]);
  const [libraryOpen, setLibraryOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const bodyRef = useRef<HTMLTextAreaElement>(null);
  // The same letter in other languages, for readers with that language (0049).
  const [variants, setVariants] = useState<Variant[]>([]);
  const setVariant = (i: number, patch: Partial<Variant>) =>
    setVariants((vs) => vs.map((v, j) => (j === i ? { ...v, ...patch } : v)));

  // The writer's own signature under a fresh letter (0049); a quoted reply goes below it.
  const prefs = usePreferences();
  const signed = useRef(false);
  useEffect(() => {
    if (signed.current || !prefs.data) return;
    signed.current = true;
    const signature = prefs.data.email_signature?.trim();
    const parts: string[] = [];
    if (signature) parts.push(signature);
    if (quote?.trim()) {
      parts.push(
        quote
          .trim()
          .split('\n')
          .map((l) => `> ${l}`)
          .join('\n'),
      );
    }
    if (parts.length > 0) setBody((b) => (b.trim() === '' ? `\n\n${parts.join('\n\n')}` : b));
  }, [prefs.data, quote]);

  const insertVariable = (name: string) => {
    const token = `{{${name}}}`;
    const el = bodyRef.current;
    if (!el) {
      setBody((b) => (b === '' ? token : `${b} ${token}`));
      return;
    }
    const start = el.selectionStart ?? body.length;
    const end = el.selectionEnd ?? body.length;
    const next = `${body.slice(0, start)}${token}${body.slice(end)}`;
    setBody(next);
    requestAnimationFrame(() => {
      el.focus();
      const pos = start + token.length;
      el.setSelectionRange(pos, pos);
    });
  };

  // Attachable documents come from the record the letter is about: the backend only
  // accepts documents owned by that order or lead.
  const orderDocs = useQuery({
    queryKey: ['order', about?.order_id ?? 0, 'documents'],
    queryFn: () => mediaApi.documents(about!.order_id!),
    enabled: about?.order_id != null,
  });
  const leadDetail = useQuery({
    queryKey: ['lead', about?.lead_id ?? 0],
    queryFn: () => leadsApi.get(about!.lead_id!),
    enabled: about?.lead_id != null,
  });
  const attachable = useMemo(
    () =>
      about?.order_id != null
        ? (orderDocs.data?.items ?? []).map((d) => ({ id: d.id, filename: d.filename }))
        : (leadDetail.data?.documents ?? []).map((d) => ({ id: d.id, filename: d.filename })),
    [about?.order_id, orderDocs.data, leadDetail.data],
  );

  // Context-free library: recent documents for newsletters and standalone letters.
  const library = useQuery({
    queryKey: ['documents-library'],
    queryFn: () => mediaApi.searchDocuments({ limit: 20 }),
    enabled: libraryOpen,
  });

  const toggleDoc = (id: number) =>
    setSelectedDocs((prev) => (prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id]));
  const toggleLibraryDoc = (id: number) =>
    setLibraryDocs((prev) => (prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id]));
  const toggleEmbed = (doc: { id: number; filename: string }) => {
    const exists = embeds.some((e) => e.id === doc.id);
    if (exists) {
      setEmbeds((prev) => prev.filter((e) => e.id !== doc.id));
      return;
    }
    setEmbeds((prev) => [...prev, doc]);
    setBody((b) => `${b}${b === '' || b.endsWith('\n') ? '' : '\n'}\n![${doc.filename}](doc:${doc.id})\n`);
  };
  const removeEmbed = (id: number) => setEmbeds((prev) => prev.filter((e) => e.id !== id));

  const templates = useQuery({
    queryKey: ['email-templates'],
    queryFn: () => emailApi.templates(),
  });
  const { data: lookups } = useLookups();
  const variables = useQuery({
    queryKey: ['email-variables'],
    queryFn: () => emailApi.variables(),
    staleTime: 10 * 60_000,
  });
  // Exactly who the server would send to now: active, not suppressed, and with one of
  // the chosen tags (none chosen is everyone). Counted by the same query as the send.
  const audienceQuery = useQuery({
    queryKey: ['newsletter-audience', tagIds],
    queryFn: () => newsletterApi.audience(tagIds),
    enabled: audience === 'newsletter',
  });
  const activeCount = audienceQuery.data?.recipients ?? 0;
  const newsletterTags = useNewsletterTags();

  const applyTemplate = (key: string) => {
    setTemplateKey(key);
    setTheme('');
    const tpl = (templates.data?.items ?? []).find((x) => x.key === key);
    if (tpl) {
      // Templates are plain text by design; Markdown plus a template is refused.
      setMarkdown(false);
      setSubject(tpl.subject);
      setBody(tpl.body);
    }
  };

  const applyTheme = (key: string) => {
    setTheme(key);
    setTemplateKey('');
    if (key === '') return;
    // Starter content lives on the server (`GET /config/lookups`): a new
    // starter or a reworded one is a server deploy, not a web deploy.
    const th = (lookups?.email_themes ?? []).find((x) => x.key === key);
    if (!th) return;
    setSubject(th.subject);
    setHero(th.hero);
    setBody(th.body);
    setMarkdown(true);
  };

  const attachmentIds = useMemo(
    () => [...selectedDocs, ...libraryDocs],
    [selectedDocs, libraryDocs],
  );
  const embedIds = useMemo(() => embeds.map((e) => e.id), [embeds]);

  const draftKey = JSON.stringify({
    order_id: about?.order_id ?? null,
    lead_id: about?.lead_id ?? null,
    partner_id: about?.partner_id ?? null,
    to: to.trim(),
    cc: cc.split(/[;,]/).map((s) => s.trim()).filter(Boolean),
    template_key: templateKey || null,
    subject: subject || null,
    hero: hero.trim() || null,
    body: body || null,
    body_markdown: markdown,
    attachment_document_ids: attachmentIds,
    embed_document_ids: embedIds,
    reply_to_inbound_id: replyToInboundId ?? null,
  });
  const debouncedKey = useDebouncedValue(draftKey, PREVIEW_DEBOUNCE_MS);
  const draft = useMemo(() => JSON.parse(debouncedKey) as ComposeRequest, [debouncedKey]);

  // A newsletter has no single recipient until it sends; preview it as addressed to
  // the office itself so the endpoint has something valid to render.
  const previewTo = audience === 'newsletter' ? 'iroda@autotherm.hu' : draft.to;
  const previewReady = subject.trim() !== '' && body.trim() !== '' && previewTo !== '';
  const preview = useQuery({
    queryKey: ['email-preview', debouncedKey],
    queryFn: () => emailApi.preview({ ...draft, to: previewTo }),
    enabled: previewReady,
    retry: false,
  });

  useEffect(() => {
    setError(null);
  }, [audience, to, subject, body]);

  const send = useMutation({
    mutationFn: async (): Promise<{ id: number; newsletterRecipients?: number }> => {
      if (audience === 'newsletter') {
        // One tracked letter per reader (own unsubscribe link, opens, clicks), now or
        // at the time picked; the answer is the send and how many readers it covers.
        const sent = await newsletterApi.schedule({
          subject: subject.trim(),
          hero: hero.trim() || null,
          body,
          body_markdown: markdown,
          attachment_document_ids: attachmentIds,
          embed_document_ids: embedIds,
          tag_ids: tagIds,
          send_at: sendAt ? new Date(sendAt).toISOString() : null,
          variants: variants
            .filter((v) => v.subject.trim() !== '' && v.body.trim() !== '')
            .map((v) => ({
              language: v.language,
              subject: v.subject.trim(),
              body: v.body,
              body_markdown: markdown,
              hero: v.hero.trim() || null,
            })),
        });
        return { id: sent.id, newsletterRecipients: sent.recipients };
      }
      const mail = await emailApi.send({
        ...draft,
        to: to.trim(),
        subject: subject.trim(),
      });
      return { id: mail.id };
    },
    onSuccess: ({ id, newsletterRecipients }) => onSent(id, newsletterRecipients),
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  const canSend =
    subject.trim() !== '' &&
    body.trim() !== '' &&
    (audience === 'newsletter' ? activeCount > 0 : to.trim() !== '') &&
    !send.isPending;

  return (
    <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
      <div className="space-y-4">
        <div className="inline-flex rounded-lg bg-steel-200/50 p-0.5 text-body" role="group" aria-label={t('audience')}>
          {(['direct', 'newsletter'] as Audience[]).map((a) => (
            <button
              key={a}
              type="button"
              onClick={() => setAudience(a)}
              aria-pressed={audience === a}
              className={`rounded-md px-3 py-1.5 ${audience === a ? 'bg-surface text-steel-900 shadow-sm' : 'text-steel-500'}`}
            >
              {t(`audience_${a}`)}
            </button>
          ))}
        </div>

        {audience === 'direct' ? (
          <>
            <div>
              <label className="label" htmlFor="compose-to">{t('to')}</label>
              <input id="compose-to" className="input" value={to} onChange={(e) => setTo(e.target.value)} placeholder="vevo@example.hu" />
            </div>
            <div>
              <label className="label" htmlFor="compose-cc">{t('cc')}</label>
              <input id="compose-cc" className="input" value={cc} onChange={(e) => setCc(e.target.value)} placeholder={t('ccHint')} />
            </div>
          </>
        ) : (
          <div className="space-y-2 rounded-lg bg-steel-200/50 px-3 py-2" data-testid="newsletter-audience">
            <div className="flex flex-wrap items-center gap-1.5">
              <span className="text-metadata text-steel-500">{t('newsletterTags')}</span>
              {tagIds.length === 0 && <span className="text-metadata font-medium">{t('newsletterEveryone')}</span>}
              <NewsletterChips
                ids={tagIds}
                all={newsletterTags.data?.items ?? []}
                onRemove={(id) => setTagIds(tagIds.filter((x) => x !== id))}
              />
              <NewsletterTagPicker label={t('newsletterPickTags')} value={tagIds} onChange={setTagIds} />
            </div>
            <p className="text-body text-steel-900">{t('newsletterAudience', { count: activeCount })}</p>
            <div className="flex flex-wrap items-center gap-2">
              <label className="text-metadata text-steel-500" htmlFor="compose-send-at">{t('newsletterSendAt')}</label>
              <input
                id="compose-send-at"
                type="datetime-local"
                className="input h-8 w-auto py-0"
                value={sendAt}
                onChange={(e) => setSendAt(e.target.value)}
              />
              {sendAt && (
                <button type="button" className="text-metadata underline" onClick={() => setSendAt('')}>
                  {t('newsletterSendNow')}
                </button>
              )}
            </div>
          </div>
        )}

        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <div>
            <label className="label" htmlFor="compose-template">{t('template')}</label>
            <select id="compose-template" className="input" value={templateKey} onChange={(e) => applyTemplate(e.target.value)}>
              <option value="">{t('noTemplate')}</option>
              {(templates.data?.items ?? []).map((tpl) => (
                <option key={tpl.key} value={tpl.key}>{tpl.name}</option>
              ))}
            </select>
          </div>
          <div>
            <label className="label" htmlFor="compose-theme">{t('theme')}</label>
            <select
              id="compose-theme"
              className="input"
              value={theme}
              disabled={templateKey !== ''}
              onChange={(e) => applyTheme(e.target.value)}
            >
              <option value="">{t('noTheme')}</option>
              {(lookups?.email_themes ?? []).map((th) => (
                <option key={th.key} value={th.key}>
                  {th.label_hu}
                </option>
              ))}
            </select>
          </div>
        </div>

        <div>
          <label className="label" htmlFor="compose-subject">{t('subject')}</label>
          <input id="compose-subject" className="input" value={subject} onChange={(e) => setSubject(e.target.value)} />
        </div>

        <div>
          <label className="label" htmlFor="compose-hero">{t('heroLabel')}</label>
          <input
            id="compose-hero"
            className="input"
            value={hero}
            onChange={(e) => setHero(e.target.value)}
            placeholder={t('heroHint')}
          />
        </div>

        <div>
          <div className="flex items-center justify-between gap-3">
            <label className="label" htmlFor="compose-body">{t('body')}</label>
            <label className="inline-flex items-center gap-2 text-metadata text-steel-500">
              <input
                type="checkbox"
                checked={markdown}
                disabled={templateKey !== ''}
                onChange={(e) => setMarkdown(e.target.checked)}
              />
              {t('markdownMode')}
            </label>
          </div>
          <textarea
            id="compose-body"
            ref={bodyRef}
            className="input mt-1 font-mono"
            rows={12}
            value={body}
            onChange={(e) => setBody(e.target.value)}
            placeholder={markdown ? t('markdownHint') : undefined}
          />
          {(variables.data?.items ?? []).length > 0 && (
            <div className="mt-2 flex flex-wrap gap-1.5" aria-label={t('variablesLabel')}>
              {(variables.data?.items ?? []).map((v) => (
                <button
                  key={v.name}
                  type="button"
                  title={v.description}
                  className="rounded-md bg-steel-200/60 px-2 py-0.5 font-mono text-metadata text-steel-900 hover:bg-steel-200"
                  onClick={() => insertVariable(v.name)}
                >
                  {`{{${v.name}}}`}
                </button>
              ))}
            </div>
          )}
          {markdown && (
            <p className="mt-1 text-metadata text-steel-500">{t('embedHint')}</p>
          )}
        </div>

        {audience === 'newsletter' && (
          <div className="space-y-3 rounded-lg border border-steel-200 p-3" data-testid="newsletter-variants">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <span className="label">{t('variantsTitle')}</span>
              <button
                type="button"
                className="btn-ghost btn-sm"
                onClick={() =>
                  setVariants((vs) => [
                    ...vs,
                    {
                      language: NEWSLETTER_LANGUAGES.find((l) => l !== 'hu' && !vs.some((v) => v.language === l)) ?? 'en',
                      subject: '',
                      hero: '',
                      body: '',
                    },
                  ])
                }
              >
                {t('variantAdd')}
              </button>
            </div>
            {variants.length === 0 && <p className="text-metadata text-steel-500">{t('variantsHint')}</p>}
            {variants.map((v, i) => (
              <div key={i} className="space-y-2 border-t border-steel-200 pt-3 first:border-t-0 first:pt-0">
                <div className="flex flex-wrap items-center gap-2">
                  <select
                    className="input h-8 w-auto py-0"
                    aria-label={t('variantLanguage')}
                    value={v.language}
                    onChange={(e) => setVariant(i, { language: e.target.value })}
                  >
                    {NEWSLETTER_LANGUAGES.filter((l) => l !== 'hu').map((l) => (
                      <option key={l} value={l}>{l.toUpperCase()}</option>
                    ))}
                  </select>
                  <input
                    className="input h-8 min-w-0 flex-1 py-0"
                    placeholder={t('subject')}
                    aria-label={`${t('subject')} (${v.language})`}
                    value={v.subject}
                    onChange={(e) => setVariant(i, { subject: e.target.value })}
                  />
                  <button
                    type="button"
                    className="btn-ghost btn-sm"
                    onClick={() => setVariants((vs) => vs.filter((_, j) => j !== i))}
                  >
                    {t('variantRemove')}
                  </button>
                </div>
                <input
                  className="input"
                  placeholder={t('heroLabel')}
                  aria-label={`${t('heroLabel')} (${v.language})`}
                  value={v.hero}
                  onChange={(e) => setVariant(i, { hero: e.target.value })}
                />
                <textarea
                  className="input font-mono"
                  rows={6}
                  aria-label={`${t('body')} (${v.language})`}
                  value={v.body}
                  onChange={(e) => setVariant(i, { body: e.target.value })}
                />
              </div>
            ))}
          </div>
        )}

        {error && (
          <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">
            {error}
          </p>
        )}
        {(about?.order_id != null || about?.lead_id != null) && attachable.length > 0 && (
          <div>
            <span className="label">{t('attachments')}</span>
            <ul className="mt-1 space-y-1">
              {attachable.map((d) => (
                <li key={d.id}>
                  <label className="inline-flex items-center gap-2 text-body">
                    <input type="checkbox" checked={selectedDocs.includes(d.id)} onChange={() => toggleDoc(d.id)} />
                    <span className="font-medium">{d.filename}</span>
                  </label>
                </li>
              ))}
            </ul>
          </div>
        )}

        <div>
          <button
            type="button"
            className="btn-ghost btn-sm"
            onClick={() => setLibraryOpen((o) => !o)}
            aria-expanded={libraryOpen}
          >
            {libraryOpen ? t('libraryHide') : t('libraryShow')}
          </button>
          {libraryOpen && (
            <div className="mt-2 rounded-lg border border-steel-200 p-3">
              {library.isLoading ? (
                <p className="text-body text-steel-500">…</p>
              ) : (library.data?.items ?? []).length === 0 ? (
                <p className="text-body text-steel-500">{t('libraryEmpty')}</p>
              ) : (
                <ul className="space-y-2">
                  {(library.data?.items ?? []).map((d) => {
                    const isImage = d.content_type.startsWith('image/');
                    const attached = libraryDocs.includes(d.id);
                    const embedded = embeds.some((e) => e.id === d.id);
                    return (
                      <li key={d.id} className="flex flex-wrap items-center gap-x-3 gap-y-1 text-body">
                        <span className="min-w-0 flex-1 font-medium">{d.filename}</span>
                        <button
                          type="button"
                          className="btn-ghost btn-sm"
                          aria-pressed={attached}
                          onClick={() => toggleLibraryDoc(d.id)}
                        >
                          {attached ? t('attachedOn') : t('attachAction')}
                        </button>
                        {isImage && (
                          <button
                            type="button"
                            className="btn-ghost btn-sm"
                            aria-pressed={embedded}
                            onClick={() => toggleEmbed({ id: d.id, filename: d.filename })}
                          >
                            {embedded ? t('embeddedOn') : t('embedAction')}
                          </button>
                        )}
                      </li>
                    );
                  })}
                </ul>
              )}
              {embeds.length > 0 && (
                <p className="mt-2 text-metadata text-steel-500">
                  {t('embeddedList', { files: embeds.map((e) => e.filename).join(', ') })}{' '}
                  {embeds.map((e) => (
                    <button key={e.id} type="button" className="underline" onClick={() => removeEmbed(e.id)}>
                      {t('removeEmbed', { file: e.filename })}
                    </button>
                  ))}
                </p>
              )}
            </div>
          )}
        </div>

        <button type="button" className="btn-primary" disabled={!canSend} onClick={() => send.mutate()}>
          {send.isPending ? '…' : audience === 'newsletter' ? t(sendAt ? 'scheduleNewsletter' : 'sendNewsletter') : t('send')}
        </button>
      </div>

      <div>
        <h2 className="text-section font-semibold">{t('previewTitle')}</h2>
        <div className="mt-3">
          {preview.isLoading ? (
            <p className="text-body text-steel-500">…</p>
          ) : preview.data ? (
            <>
              {(preview.data.unresolved.length > 0 || preview.data.recipient_suppressed) && (
                <p className="mb-2 rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="note">
                  {preview.data.recipient_suppressed && <>{t('recipientSuppressed')} </>}
                  {preview.data.unresolved.length > 0 && (
                    <>{t('unresolvedVariables')}: {preview.data.unresolved.join(', ')}</>
                  )}
                </p>
              )}
              <iframe
                title={t('previewTitle')}
                sandbox=""
                srcDoc={preview.data.body_html}
                className="h-[560px] w-full rounded-lg border border-steel-200 bg-white"
              />
            </>
          ) : (
            <p className="text-body text-steel-500">{t('previewEmpty')}</p>
          )}
        </div>
      </div>
    </div>
  );
}
