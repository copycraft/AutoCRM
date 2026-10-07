'use client';

// A template as it would read for a real order, lead or partner, while it is being edited
// (0049). Pick the record by searching; nothing is saved or sent.

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useQuery } from '@tanstack/react-query';
import { searchApi, templatePreviewApi } from '@/lib/api/endpoints';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { errorMessage } from '@/lib/api/errors';

type Target = { kind: 'order' | 'lead' | 'partner'; id: number; label: string };

export function TemplatePreview({ subject, body }: { subject: string; body: string }) {
  const t = useTranslations('templatesPage');
  const ter = useTranslations('errors');
  const [q, setQ] = useState('');
  const [target, setTarget] = useState<Target | null>(null);
  const debouncedQ = useDebouncedValue(q, 300);
  const hits = useQuery({
    queryKey: ['template-preview-search', debouncedQ],
    queryFn: () => searchApi.global(debouncedQ),
    enabled: debouncedQ.trim().length >= 2,
  });
  const debouncedSubject = useDebouncedValue(subject, 500);
  const debouncedBody = useDebouncedValue(body, 500);
  const preview = useQuery({
    queryKey: ['template-preview', target?.kind, target?.id, debouncedSubject, debouncedBody],
    queryFn: () =>
      templatePreviewApi.preview({
        subject: debouncedSubject,
        body: debouncedBody,
        order_id: target?.kind === 'order' ? target.id : null,
        lead_id: target?.kind === 'lead' ? target.id : null,
        partner_id: target?.kind === 'partner' ? target.id : null,
      }),
    enabled: debouncedSubject.trim() !== '' && debouncedBody.trim() !== '',
    retry: false,
  });

  const options: Target[] = hits.data
    ? [
        ...hits.data.orders.slice(0, 5).map((o) => ({ kind: 'order' as const, id: o.id, label: `${o.number} ${o.title}` })),
        ...hits.data.leads.slice(0, 5).map((l) => ({ kind: 'lead' as const, id: l.id, label: l.title })),
        ...hits.data.partners.slice(0, 5).map((p) => ({ kind: 'partner' as const, id: p.id, label: p.name })),
      ]
    : [];

  return (
    <div className="space-y-2 rounded-lg border border-steel-200 p-3">
      <p className="label">{t('previewTitle')}</p>
      <div className="flex flex-wrap items-center gap-2">
        <input
          className="input h-8 min-w-0 flex-1 py-0"
          placeholder={t('previewSearch')}
          aria-label={t('previewSearch')}
          value={q}
          onChange={(e) => setQ(e.target.value)}
        />
        {target && (
          <button type="button" className="text-metadata underline" onClick={() => setTarget(null)}>
            {t('previewClear')}
          </button>
        )}
      </div>
      {q.trim().length >= 2 && options.length > 0 && !target && (
        <ul className="max-h-40 overflow-y-auto rounded border border-steel-200 text-body">
          {options.map((o) => (
            <li key={`${o.kind}-${o.id}`}>
              <button
                type="button"
                className="w-full px-2 py-1 text-left hover:bg-steel-100"
                onClick={() => {
                  setTarget(o);
                  setQ('');
                }}
              >
                <span className="mr-2 text-metadata text-steel-500">{t(`previewKinds.${o.kind}`)}</span>
                {o.label}
              </button>
            </li>
          ))}
        </ul>
      )}
      <p className="text-metadata text-steel-500">
        {target ? t('previewFor', { kind: t(`previewKinds.${target.kind}`), label: target.label }) : t('previewNone')}
      </p>
      {preview.isError ? (
        <p className="text-metadata text-signal">{errorMessage(preview.error, ter, ter('unknownError'))}</p>
      ) : preview.data ? (
        <>
          {preview.data.unresolved.length > 0 && (
            <p className="rounded bg-steel-200/50 px-2 py-1 text-metadata text-steel-900">
              {t('previewMissing')}: {preview.data.unresolved.join(', ')}
            </p>
          )}
          <p className="text-body font-medium">{preview.data.subject}</p>
          <iframe
            title={t('previewTitle')}
            sandbox=""
            srcDoc={preview.data.body_html}
            className="h-72 w-full rounded border border-steel-200 bg-white"
          />
        </>
      ) : null}
    </div>
  );
}
