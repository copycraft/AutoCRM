'use client';

// The files of an order or a lead: drawings, CAD, certificates, quotation PDFs. Upload
// from the desk, look inside without downloading (PDF, picture, video), keep earlier
// versions when a drawing is replaced, and see a drawing's thumbnail before opening it.

import { useRef, useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Download, Eye, FileText, FileUp, History, Trash2, Upload, X } from 'lucide-react';
import { mediaApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { useToast } from '@/components/ui/Toasts';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { LoadingState } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import { uploadFile } from '@/lib/upload';
import type { DocumentView } from '@/lib/api/types';
import type { components } from '@/lib/api/schema.gen';

type DocumentKind = components['schemas']['DocumentKind'];

/** What a person files by hand. Invoices and proformas are filed by the server. */
const KINDS: DocumentKind[] = ['design', 'cad', 'certificate', 'other'];

function sizeText(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} kB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export function DocumentsPanel({
  owner,
  canUpload,
  canDelete,
}: {
  owner: { order: number } | { lead: number };
  canUpload: boolean;
  canDelete: boolean;
}) {
  const t = useTranslations('media');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const { toast } = useToast();
  const key = 'order' in owner ? qk.documents(owner.order) : qk.leadDocuments(owner.lead);
  const query = useQuery({
    queryKey: key,
    queryFn: () => ('order' in owner ? mediaApi.documents(owner.order) : mediaApi.leadDocuments(owner.lead)),
  });
  const [kind, setKind] = useState<DocumentKind>('order' in owner ? 'design' : 'other');
  const [busy, setBusy] = useState(false);
  const [preview, setPreview] = useState<DocumentView | null>(null);
  const [historyOf, setHistoryOf] = useState<number | null>(null);
  const newFile = useRef<HTMLInputElement>(null);
  const versionFile = useRef<HTMLInputElement>(null);
  const [replacing, setReplacing] = useState<DocumentView | null>(null);

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: key });
    void qc.invalidateQueries({ queryKey: ['document'] });
    if ('lead' in owner) void qc.invalidateQueries({ queryKey: qk.lead(owner.lead) });
  };

  const upload = async (files: File[], replaces?: DocumentView) => {
    setBusy(true);
    try {
      for (const file of files) {
        const r = await uploadFile(owner, file, {
          type: 'document',
          kind: replaces?.kind ?? kind,
          ...(replaces ? { replaces: replaces.id } : {}),
        });
        if (r.status === 'already_uploaded') toast('info', t('alreadyThere', { count: 1 }));
        else toast('success', replaces ? t('versionUploaded', { name: file.name }) : t('fileUploaded', { name: file.name }));
      }
    } catch (e) {
      toast('error', t('uploadFailed', { name: files[0]?.name ?? '' }), errorMessage(e, ter, ter('unknownError')));
    } finally {
      setBusy(false);
      setReplacing(null);
      refresh();
    }
  };

  const remove = useMutation({
    mutationFn: (id: number) => mediaApi.deleteDocument(id),
    onSuccess: refresh,
    onError: (e) => toast('error', errorMessage(e, ter, ter('unknownError'))),
  });

  const download = async (id: number) => {
    try {
      const { url } = await mediaApi.downloadDocument(id);
      window.location.href = url;
    } catch (e) {
      toast('error', errorMessage(e, ter, ter('unknownError')));
    }
  };

  const docs = query.data?.items ?? [];
  return (
    <div
      className="space-y-3"
      onDragOver={(e) => {
        if (canUpload && e.dataTransfer.types.includes('Files')) e.preventDefault();
      }}
      onDrop={(e) => {
        if (!canUpload || !e.dataTransfer.files.length) return;
        e.preventDefault();
        void upload(Array.from(e.dataTransfer.files));
      }}
    >
      {canUpload && (
        <div className="flex flex-wrap items-center gap-2 rounded-lg border border-dashed border-steel-200 p-3">
          <FileUp className="h-5 w-5 text-steel-500" aria-hidden />
          <select className="input h-8 w-auto py-0" value={kind} onChange={(e) => setKind(e.target.value as DocumentKind)} aria-label={t('kind')}>
            {KINDS.map((k) => (
              <option key={k} value={k}>{t(`kinds.${k}`)}</option>
            ))}
          </select>
          <button type="button" className="btn-secondary btn-sm" onClick={() => newFile.current?.click()} disabled={busy}>
            <Upload className="h-4 w-4" aria-hidden /> {busy ? t('uploading') : t('addFile')}
          </button>
          <span className="text-metadata text-steel-500">{t('dropFiles')}</span>
          <input
            ref={newFile}
            type="file"
            multiple
            hidden
            onChange={(e) => {
              if (e.target.files?.length) void upload(Array.from(e.target.files));
              e.target.value = '';
            }}
          />
          <input
            ref={versionFile}
            type="file"
            hidden
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f && replacing) void upload([f], replacing);
              e.target.value = '';
            }}
          />
        </div>
      )}

      {query.isPending ? (
        <LoadingState />
      ) : query.isError ? (
        <ErrorState error={query.error} onRetry={() => void query.refetch()} />
      ) : docs.length === 0 ? (
        <p className="text-body text-steel-500">{t('noFiles')}</p>
      ) : (
        <ul className="divide-y divide-steel-200 rounded-lg border border-steel-200">
          {docs.map((d) => (
            <li key={d.id} className="p-3">
              <div className="flex flex-wrap items-center gap-3">
                {d.thumb_url ? (
                  <img src={d.thumb_url} alt="" className="h-14 w-20 rounded border border-steel-200 bg-surface object-contain" loading="lazy" />
                ) : (
                  <span className="flex h-14 w-20 items-center justify-center rounded border border-steel-200 bg-panel">
                    <FileText className="h-6 w-6 text-steel-500" aria-hidden />
                  </span>
                )}
                <div className="min-w-0 flex-1">
                  <p className="truncate text-body font-medium">{d.filename}</p>
                  <p className="flex flex-wrap items-center gap-2 text-metadata text-steel-500">
                    <StatusBadge tone="steel">{t(`kinds.${d.kind}`)}</StatusBadge>
                    {d.version > 1 && <StatusBadge tone="cold">v{d.version}</StatusBadge>}
                    <DateDisplay value={d.uploaded_at} /> · {sizeText(d.byte_size)}
                    {d.valid_until && (
                      <>
                        · {t('validUntil')} <DateDisplay value={d.valid_until} />
                      </>
                    )}
                  </p>
                </div>
                <div className="flex flex-wrap gap-1">
                  <button type="button" className="btn-ghost btn-sm" onClick={() => setPreview(d)} title={t('preview')}>
                    <Eye className="h-4 w-4" aria-hidden /> <span className="sr-only">{t('preview')}</span>
                  </button>
                  <button type="button" className="btn-ghost btn-sm" onClick={() => void download(d.id)} title={t('download')}>
                    <Download className="h-4 w-4" aria-hidden /> <span className="sr-only">{t('download')}</span>
                  </button>
                  {d.version > 1 && (
                    <button
                      type="button"
                      className="btn-ghost btn-sm"
                      onClick={() => setHistoryOf(historyOf === d.id ? null : d.id)}
                      aria-expanded={historyOf === d.id}
                      title={t('versions')}
                    >
                      <History className="h-4 w-4" aria-hidden /> <span className="sr-only">{t('versions')}</span>
                    </button>
                  )}
                  {canUpload && d.kind !== 'invoice' && d.kind !== 'proforma' && (
                    <button
                      type="button"
                      className="btn-ghost btn-sm"
                      disabled={busy}
                      onClick={() => {
                        setReplacing(d);
                        versionFile.current?.click();
                      }}
                    >
                      {t('newVersion')}
                    </button>
                  )}
                  {canDelete && d.kind !== 'invoice' && d.kind !== 'proforma' && (
                    <button
                      type="button"
                      className="btn-ghost btn-sm text-signal"
                      onClick={() => {
                        if (window.confirm(t('deleteFileConfirm', { name: d.filename }))) remove.mutate(d.id);
                      }}
                      title={t('deleteFile')}
                    >
                      <Trash2 className="h-4 w-4" aria-hidden /> <span className="sr-only">{t('deleteFile')}</span>
                    </button>
                  )}
                </div>
              </div>
              {d.thumb_error && <p className="mt-1 text-metadata text-steel-500">{t('noThumb')}: {d.thumb_error}</p>}
              {historyOf === d.id && <VersionList id={d.id} onPreview={setPreview} onDownload={(id) => void download(id)} />}
            </li>
          ))}
        </ul>
      )}

      {preview && <PreviewDialog doc={preview} onClose={() => setPreview(null)} />}
    </div>
  );
}

function VersionList({
  id,
  onPreview,
  onDownload,
}: {
  id: number;
  onPreview: (d: DocumentView) => void;
  onDownload: (id: number) => void;
}) {
  const t = useTranslations('media');
  const query = useQuery({ queryKey: qk.documentVersions(id), queryFn: () => mediaApi.documentVersions(id) });
  if (query.isPending) return <LoadingState />;
  if (query.isError) return <ErrorState error={query.error} onRetry={() => void query.refetch()} />;
  return (
    <ul className="mt-2 space-y-1 border-l-2 border-steel-200 pl-3">
      {query.data.items.map((v) => (
        <li key={v.id} className="flex flex-wrap items-center gap-2 text-metadata">
          <span className="font-mono">v{v.version}</span>
          <span className="truncate">{v.filename}</span>
          <DateDisplay value={v.uploaded_at} />
          {v.superseded_at ? <StatusBadge tone="muted">{t('superseded')}</StatusBadge> : <StatusBadge tone="done">{t('current')}</StatusBadge>}
          <button type="button" className="btn-ghost btn-sm" onClick={() => onPreview(v)}>{t('preview')}</button>
          <button type="button" className="btn-ghost btn-sm" onClick={() => onDownload(v.id)}>{t('download')}</button>
        </li>
      ))}
    </ul>
  );
}

/** PDFs, pictures and video open in the page; anything else offers the download. */
export function PreviewDialog({ doc, onClose }: { doc: { id: number; filename: string; content_type: string }; onClose: () => void }) {
  const t = useTranslations('media');
  const query = useQuery({ queryKey: ['document', doc.id, 'preview'], queryFn: () => mediaApi.previewDocument(doc.id) });
  const type = (query.data?.content_type ?? doc.content_type).toLowerCase();
  const name = doc.filename.toLowerCase();
  const isPdf = type === 'application/pdf' || name.endsWith('.pdf');
  const isImage = type.startsWith('image/') && !/\.(dxf|dwg|heic|heif)$/.test(name);
  const isVideo = type.startsWith('video/');
  return (
    <Dialog.Root open onOpenChange={(o) => !o && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-steel-900/70" />
        <Dialog.Content className="fixed inset-2 z-50 flex flex-col overflow-hidden rounded-xl bg-surface lg:inset-8">
          <div className="flex items-center justify-between gap-2 border-b border-steel-200 px-4 py-2">
            <Dialog.Title className="truncate text-body font-semibold">{doc.filename}</Dialog.Title>
            <div className="flex gap-2">
              {query.data && (
                <a className="btn-secondary btn-sm" href={query.data.url} target="_blank" rel="noopener noreferrer">
                  {t('openNewTab')}
                </a>
              )}
              <Dialog.Close className="btn-ghost btn-sm" aria-label={t('close')}>
                <X className="h-4 w-4" aria-hidden />
              </Dialog.Close>
            </div>
          </div>
          <div className="flex min-h-0 flex-1 items-center justify-center bg-panel">
            {query.isPending ? (
              <LoadingState />
            ) : query.isError ? (
              <ErrorState error={query.error} onRetry={() => void query.refetch()} />
            ) : isPdf ? (
              <iframe src={query.data.url} title={doc.filename} className="h-full w-full border-0" />
            ) : isImage ? (
              <img src={query.data.url} alt={doc.filename} className="max-h-full max-w-full object-contain" />
            ) : isVideo ? (
              <video src={query.data.url} controls className="max-h-full max-w-full" />
            ) : (
              <p className="p-6 text-center text-body text-steel-500">{t('noInlinePreview')}</p>
            )}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
