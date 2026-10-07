'use client';

// Sablonok: every email template, one tab per area (Értékesítés, Projektek, Számlázó...),
// filtered by folder on the side, the way MiniCRM laid them out. Click a template to edit
// it, copy it, or move it to the bin. The sales tab also holds the quote follow-up sequence.

import { useMemo, useRef, useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Copy, Plus, RotateCcw, Search, Trash2, X } from 'lucide-react';
import { AppShell } from '@/components/layout/AppShell';
import { PageHeader } from '@/components/layout/PageHeader';
import { ErrorState } from '@/components/ui/ErrorState';
import { useUrlState } from '@/hooks/useUrlState';
import { emailApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { canAdmin, useAuth } from '@/lib/auth/context';
import { cn } from '@/lib/utils/format';
import { FollowupSteps } from '@/components/followups/FollowupSteps';
import { TemplatePreview } from '@/components/email/TemplatePreview';
import type { EmailTemplate } from '@/lib/api/types';

const CATEGORIES = ['sales', 'projects', 'billing', 'marketing', 'hr', 'general'] as const;
const FOLDERS = ['customer', 'workflow', 'design'] as const;
const BIN = 'bin';

export default function TemplatesPage() {
  const t = useTranslations('templatesPage');
  const tn = useTranslations('navigation');
  const { user } = useAuth();
  // Editing templates is configuration (the server asks for it too).
  const editable = canAdmin(user);
  const qc = useQueryClient();
  const [tab, setTab] = useUrlState('tab', 'sales');
  const [folder, setFolder] = useUrlState('folder', '');
  const [q, setQ] = useState('');
  const [open, setOpen] = useState<EmailTemplate | 'new' | null>(null);

  const list = useQuery({ queryKey: ['email-templates'], queryFn: () => emailApi.templates() });
  const all = useMemo(() => list.data?.items ?? [], [list.data]);
  const inTab = all.filter((x) => (tab === BIN ? !!x.archived_at : !x.archived_at && x.category === tab));
  const needle = q.trim().toLowerCase();
  const rows = inTab
    .filter((x) => !folder || x.folder === folder)
    .filter((x) => !needle || `${x.name} ${x.subject}`.toLowerCase().includes(needle))
    .sort((a, b) => a.name.localeCompare(b.name, 'hu'));
  const count = (c: string) => all.filter((x) => (c === BIN ? !!x.archived_at : !x.archived_at && x.category === c)).length;

  return (
    <AppShell>
      <PageHeader
        title={tn('templates')}
        actions={
          editable && (
            <button type="button" className="btn-primary btn-sm" onClick={() => setOpen('new')}>
              <Plus className="h-4 w-4" aria-hidden />
              {t('new')}
            </button>
          )
        }
      />

      <div role="tablist" className="flex flex-wrap gap-1 border-b border-steel-200">
        {[...CATEGORIES, BIN].map((c) => (
          <button
            key={c}
            role="tab"
            aria-selected={tab === c}
            onClick={() => {
              setTab(c);
              setFolder('');
            }}
            className={cn(
              '-mb-px border-b-2 px-3 py-2 text-body font-medium',
              tab === c ? 'border-steel-900 text-steel-900' : 'border-transparent text-steel-500 hover:text-steel-900',
              c === BIN && 'ml-auto',
            )}
          >
            {c === BIN && <Trash2 className="mr-1 inline h-4 w-4" aria-hidden />}
            {t(`tabs.${c}`)} <span className="font-mono text-metadata opacity-70">{count(c)}</span>
          </button>
        ))}
      </div>

      {tab === 'sales' && <FollowupSteps editable={editable} />}
      {tab === 'billing' && <FollowupSteps editable={editable} kind="invoice" />}

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-[minmax(0,1fr)_16rem]">
        <div className="min-w-0">
          {list.isError ? (
            <ErrorState error={list.error} onRetry={() => void list.refetch()} />
          ) : (
            <div className="card overflow-x-auto">
              <table className="w-full text-body">
                <thead className="border-b border-steel-200 text-left text-metadata text-steel-500">
                  <tr>
                    <th className="px-4 py-2">{t('name')}</th>
                    <th className="px-4 py-2">{t('type')}</th>
                    <th className="px-4 py-2">{t('folder')}</th>
                    <th className="px-4 py-2">{t('usage')}</th>
                  </tr>
                </thead>
                <tbody>
                  {rows.map((x) => (
                    <tr
                      key={x.id}
                      className="cursor-pointer border-b border-steel-200 last:border-b-0 hover:bg-steel-200/30"
                      onClick={() => setOpen(x)}
                    >
                      <td className="px-4 py-2.5 font-medium">{x.name}</td>
                      <td className="px-4 py-2.5">
                        <span className="rounded-full bg-cold/15 px-2 py-0.5 text-metadata font-medium text-cold">Email</span>
                      </td>
                      <td className="px-4 py-2.5 text-steel-500">{t(`folders.${x.folder}`)}</td>
                      <td className="px-4 py-2.5 text-metadata text-steel-500">{x.is_automatic ? t('automatic') : t('manual')}</td>
                    </tr>
                  ))}
                  {!list.isPending && rows.length === 0 && (
                    <tr>
                      <td colSpan={4} className="px-4 py-6 text-center text-steel-500">{t('empty')}</td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          )}
          <p className="mt-2 text-metadata text-steel-500">{t('countLine', { n: rows.length, total: inTab.length })}</p>
        </div>

        <aside className="space-y-4">
          <div className="relative">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-steel-500" aria-hidden />
            <input className="input pl-9" placeholder={t('search')} aria-label={t('search')} value={q} onChange={(e) => setQ(e.target.value)} />
          </div>
          <nav className="card p-3 text-body" aria-label={t('folder')}>
            <h3 className="mb-2 font-semibold">{t('folder')}</h3>
            <ul className="space-y-0.5">
              {['', ...FOLDERS].map((f) => (
                <li key={f || 'all'}>
                  <button
                    type="button"
                    className={cn('w-full rounded px-2 py-1 text-left hover:bg-steel-200/40', folder === f && 'bg-steel-200/60 font-semibold')}
                    onClick={() => setFolder(f)}
                  >
                    {f ? t(`folders.${f}`) : t('allFolders')}{' '}
                    <span className="font-mono text-metadata text-steel-500">
                      ({f ? inTab.filter((x) => x.folder === f).length : inTab.length})
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </nav>
        </aside>
      </div>

      {open && (
        <TemplateEditor
          key={open === 'new' ? 'new' : open.id}
          template={open === 'new' ? null : open}
          defaultCategory={tab === BIN ? 'general' : tab}
          editable={editable}
          onClose={() => setOpen(null)}
          onSaved={(saved) => {
            void qc.invalidateQueries({ queryKey: ['email-templates'] });
            setOpen(saved);
          }}
        />
      )}
    </AppShell>
  );
}

function TemplateEditor({
  template,
  defaultCategory,
  editable,
  onClose,
  onSaved,
}: {
  template: EmailTemplate | null;
  defaultCategory: string;
  editable: boolean;
  onClose: () => void;
  onSaved: (t: EmailTemplate | null) => void;
}) {
  const t = useTranslations('templatesPage');
  const ter = useTranslations('errors');
  const [name, setName] = useState(template?.name ?? '');
  const [subject, setSubject] = useState(template?.subject ?? '');
  const [body, setBody] = useState(template?.body ?? '');
  const [category, setCategory] = useState(template?.category ?? defaultCategory);
  const [folder, setFolder] = useState(template?.folder ?? 'customer');
  const [error, setError] = useState<string | null>(null);
  const bodyRef = useRef<HTMLTextAreaElement>(null);
  const variables = useQuery({ queryKey: ['email-variables'], queryFn: () => emailApi.variables(), staleTime: 10 * 60_000 });
  const onError = (e: unknown) => setError(errorMessage(e, ter, ter('unknownError')));

  const save = useMutation({
    mutationFn: () =>
      template
        ? emailApi.updateTemplate(template.id, { name, subject, body, category, folder })
        : emailApi.createTemplate({ name, subject, body, category, folder }),
    onSuccess: (saved) => {
      setError(null);
      onSaved(saved);
    },
    onError,
  });
  const copy = useMutation({ mutationFn: () => emailApi.copyTemplate(template!.id), onSuccess: onSaved, onError });
  const bin = useMutation({
    mutationFn: (archived: boolean) => emailApi.updateTemplate(template!.id, { archived }),
    onSuccess: () => onSaved(null),
    onError,
  });

  const insert = (name: string) => {
    const token = `{{${name}}}`;
    const el = bodyRef.current;
    if (!el) return setBody((b) => b + token);
    const start = el.selectionStart ?? body.length;
    const end = el.selectionEnd ?? body.length;
    setBody(body.slice(0, start) + token + body.slice(end));
    requestAnimationFrame(() => {
      el.focus();
      el.setSelectionRange(start + token.length, start + token.length);
    });
  };

  return (
    <aside
      className="fixed inset-y-0 right-0 z-50 flex w-full max-w-2xl flex-col border-l border-steel-200 bg-surface shadow-2xl"
      role="dialog"
      aria-label={template?.name ?? t('new')}
      onKeyDown={(e) => e.key === 'Escape' && onClose()}
    >
      <header className="flex items-center gap-2 border-b border-steel-200 px-5 py-3">
        <div className="min-w-0 flex-1">
          <p className="truncate font-semibold">{template?.name ?? t('new')}</p>
          {template && (
            <p className="truncate font-mono text-metadata text-steel-500">
              {template.key}
              {template.is_automatic ? ` · ${t('automaticHint')}` : ''}
            </p>
          )}
        </div>
        <button type="button" className="btn-ghost btn-sm" onClick={onClose} aria-label={t('close')}>
          <X className="h-4 w-4" aria-hidden />
        </button>
      </header>
      <form
        className="flex-1 space-y-4 overflow-y-auto px-5 py-4"
        onSubmit={(e) => {
          e.preventDefault();
          save.mutate();
        }}
      >
        <div>
          <label className="label" htmlFor="tpl-name">{t('name')}</label>
          <input id="tpl-name" className="input" value={name} disabled={!editable} onChange={(e) => setName(e.target.value)} />
        </div>
        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className="label" htmlFor="tpl-cat">{t('area')}</label>
            <select id="tpl-cat" className="input" value={category} disabled={!editable} onChange={(e) => setCategory(e.target.value)}>
              {CATEGORIES.map((c) => (
                <option key={c} value={c}>{t(`tabs.${c}`)}</option>
              ))}
            </select>
          </div>
          <div>
            <label className="label" htmlFor="tpl-folder">{t('folder')}</label>
            <select id="tpl-folder" className="input" value={folder} disabled={!editable} onChange={(e) => setFolder(e.target.value)}>
              {FOLDERS.map((f) => (
                <option key={f} value={f}>{t(`folders.${f}`)}</option>
              ))}
            </select>
          </div>
        </div>
        <div>
          <label className="label" htmlFor="tpl-subject">{t('subject')}</label>
          <input id="tpl-subject" className="input" value={subject} disabled={!editable} onChange={(e) => setSubject(e.target.value)} />
        </div>
        <div>
          <label className="label" htmlFor="tpl-body">{t('body')}</label>
          <textarea
            id="tpl-body"
            ref={bodyRef}
            className="input min-h-[18rem] font-mono text-metadata"
            value={body}
            disabled={!editable}
            onChange={(e) => setBody(e.target.value)}
          />
        </div>
        {editable && (
          <div>
            <p className="label">{t('variables')}</p>
            <div className="flex flex-wrap gap-1">
              {(variables.data?.items ?? []).map((v) => (
                <button
                  key={v.name}
                  type="button"
                  className="rounded bg-steel-200/60 px-1.5 py-0.5 font-mono text-[11px] hover:bg-steel-200"
                  title={v.description}
                  onClick={() => insert(v.name)}
                >
                  {`{{${v.name}}}`}
                </button>
              ))}
            </div>
          </div>
        )}
        <TemplatePreview subject={subject} body={body} />
        {error && <p className="text-body text-signal" role="alert">{error}</p>}
      </form>
      {editable && (
        <footer className="flex flex-wrap items-center gap-2 border-t border-steel-200 px-5 py-3">
          {template && !template.archived_at && (
            <button type="button" className="btn-ghost btn-sm" disabled={bin.isPending} onClick={() => bin.mutate(true)}>
              <Trash2 className="h-4 w-4" aria-hidden />
              {t('toBin')}
            </button>
          )}
          {template?.archived_at && (
            <button type="button" className="btn-ghost btn-sm" disabled={bin.isPending} onClick={() => bin.mutate(false)}>
              <RotateCcw className="h-4 w-4" aria-hidden />
              {t('restore')}
            </button>
          )}
          {template && (
            <button type="button" className="btn-secondary btn-sm" disabled={copy.isPending} onClick={() => copy.mutate()}>
              <Copy className="h-4 w-4" aria-hidden />
              {t('copy')}
            </button>
          )}
          <button
            type="button"
            className="btn-primary btn-sm ml-auto"
            disabled={save.isPending || !name.trim() || !subject.trim() || !body.trim()}
            onClick={() => save.mutate()}
          >
            {save.isPending ? '…' : t('save')}
          </button>
        </footer>
      )}
    </aside>
  );
}
