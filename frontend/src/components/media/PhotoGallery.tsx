'use client';

// An order's photos on the web: browse by category, open one large with its caption and
// drawings, add photos, download them as a ZIP, compare two side by side. Intake photos
// are evidence: they can be captioned and drawn over (an overlay), never deleted, and carry
// the time-stamping authority's stamp when one is configured.

import { useEffect, useMemo, useRef, useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  ChevronLeft,
  ChevronRight,
  Download,
  FileArchive,
  ImagePlus,
  Lock,
  PenLine,
  ShieldCheck,
  SplitSquareHorizontal,
  Trash2,
  X,
} from 'lucide-react';
import { mediaApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { lookupLabel, useLookups } from '@/hooks/useLookups';
import { useToast } from '@/components/ui/Toasts';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { LoadingState } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import { cn } from '@/lib/utils/format';
import { uploadFile } from '@/lib/upload';
import { AnnotationEditor, ShapesSvg, parseShapes, type Shape } from './Annotations';
import { BeforeAfter } from './BeforeAfter';
import type { ImageCategory, ImageView, Vehicle } from '@/lib/api/types';

/** Categories a person may file a photo under by hand: the server's attachable ones plus
 *  the current stage's default (in MEO: completion). Evidence never comes from here. */
export function uploadCategories(
  lookups: ReturnType<typeof useLookups>['data'],
  stageDefault: ImageCategory | null | undefined,
): ImageCategory[] {
  const attachable = (lookups?.image_categories ?? [])
    .filter((c) => c.attachable)
    .map((c) => c.key as ImageCategory);
  const list = attachable.length ? attachable : (['production'] as ImageCategory[]);
  if (stageDefault && stageDefault !== 'intake' && stageDefault !== 'inspection' && !list.includes(stageDefault)) {
    list.unshift(stageDefault);
  }
  return list;
}

export function PhotoGallery({
  orderId,
  vehicles,
  stageCategory,
  canUpload,
  canDelete,
  canViewOriginal,
}: {
  orderId: number;
  vehicles: Vehicle[];
  stageCategory: ImageCategory | null | undefined;
  canUpload: boolean;
  canDelete: boolean;
  canViewOriginal: boolean;
}) {
  const t = useTranslations('media');
  const ti = useTranslations('images');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const { toast } = useToast();
  const { data: lookups } = useLookups();
  const [filter, setFilter] = useState<string>('all');
  const [open, setOpen] = useState<number | null>(null);
  const [comparing, setComparing] = useState(false);
  const [picked, setPicked] = useState<number[]>([]);
  const [uploading, setUploading] = useState<{ done: number; total: number } | null>(null);
  const categories = uploadCategories(lookups, stageCategory);
  const [category, setCategory] = useState<ImageCategory>(categories[0] ?? 'production');
  useEffect(() => {
    // Follow the stage's default as the order moves on, until someone picks by hand.
    if (stageCategory && categories.includes(stageCategory)) setCategory(stageCategory);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [stageCategory]);
  const fileInput = useRef<HTMLInputElement>(null);

  const query = useQuery({ queryKey: qk.images(orderId), queryFn: () => mediaApi.images(orderId) });
  const all = query.data?.items ?? [];
  const shown = filter === 'all' ? all : all.filter((i) => i.category === filter);
  const counts = useMemo(() => {
    const out = new Map<string, number>();
    for (const i of all) out.set(i.category, (out.get(i.category) ?? 0) + 1);
    return out;
  }, [all]);

  const upload = async (files: FileList | File[]) => {
    const list = Array.from(files).filter((f) => f.size > 0);
    if (!list.length) return;
    setUploading({ done: 0, total: list.length });
    let failed = 0;
    let skipped = 0;
    for (const [i, file] of list.entries()) {
      try {
        const r = await uploadFile({ order: orderId }, file, { type: 'image', category });
        if (r.status !== 'created') skipped += 1;
      } catch (e) {
        failed += 1;
        toast('error', t('uploadFailed', { name: file.name }), errorMessage(e, ter, ter('unknownError')));
      }
      setUploading({ done: i + 1, total: list.length });
    }
    setUploading(null);
    void qc.invalidateQueries({ queryKey: ['order', orderId] });
    const added = list.length - failed - skipped;
    if (added > 0) toast('success', t('uploaded', { count: added }));
    if (skipped > 0) toast('info', t('alreadyThere', { count: skipped }));
  };

  const togglePick = (id: number) =>
    setPicked((p) => (p.includes(id) ? p.filter((x) => x !== id) : [...p.slice(-1), id]));
  const pair = picked.map((id) => all.find((i) => i.id === id)).filter(Boolean) as ImageView[];
  const [first, second] = pair;

  return (
    <section
      className="space-y-4"
      onDragOver={(e) => {
        if (canUpload && e.dataTransfer.types.includes('Files')) e.preventDefault();
      }}
      onDrop={(e) => {
        if (!canUpload || !e.dataTransfer.files.length) return;
        e.preventDefault();
        void upload(e.dataTransfer.files);
      }}
    >
      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          className={cn('btn-sm', filter === 'all' ? 'btn-primary' : 'btn-ghost')}
          onClick={() => setFilter('all')}
        >
          {t('all')} <span className="font-mono opacity-70">{all.length}</span>
        </button>
        {[...counts.entries()].map(([key, n]) => (
          <button
            key={key}
            type="button"
            className={cn('btn-sm', filter === key ? 'btn-primary' : 'btn-ghost')}
            onClick={() => setFilter(key)}
          >
            {lookupLabel(lookups?.image_categories, key)} <span className="font-mono opacity-70">{n}</span>
          </button>
        ))}
        <span className="flex-1" />
        <button
          type="button"
          className={cn('btn-sm', comparing ? 'btn-primary' : 'btn-secondary')}
          onClick={() => {
            setComparing((c) => !c);
            setPicked([]);
          }}
          disabled={all.length < 2}
        >
          <SplitSquareHorizontal className="h-4 w-4" aria-hidden />
          {t('compare')}
        </button>
        <a
          className={cn('btn-secondary btn-sm', !all.length && 'pointer-events-none opacity-50')}
          href={mediaApi.zipUrl(orderId, 'display', filter === 'all' ? undefined : filter)}
          download
        >
          <FileArchive className="h-4 w-4" aria-hidden />
          {t('zip')}
        </a>
        {canViewOriginal && (
          <a
            className={cn('btn-ghost btn-sm', !all.length && 'pointer-events-none opacity-50')}
            href={mediaApi.zipUrl(orderId, 'original', filter === 'all' ? undefined : filter)}
            download
            title={t('zipOriginalHint')}
          >
            {t('zipOriginal')}
          </a>
        )}
      </div>

      {canUpload && (
        <div className="flex flex-wrap items-center gap-2 rounded-lg border border-dashed border-steel-200 p-3">
          <ImagePlus className="h-5 w-5 text-steel-500" aria-hidden />
          <label className="text-metadata text-steel-500" htmlFor={`gallery-cat-${orderId}`}>
            {t('uploadAs')}
          </label>
          <select
            id={`gallery-cat-${orderId}`}
            className="input h-8 w-auto py-0"
            value={category}
            onChange={(e) => setCategory(e.target.value as ImageCategory)}
          >
            {categories.map((c) => (
              <option key={c} value={c}>
                {lookupLabel(lookups?.image_categories, c)}
                {c === stageCategory ? ` · ${t('stageDefault')}` : ''}
              </option>
            ))}
          </select>
          <button
            type="button"
            className="btn-secondary btn-sm"
            onClick={() => fileInput.current?.click()}
            disabled={!!uploading}
          >
            {uploading ? t('uploadingCount', uploading) : ti('selectFiles')}
          </button>
          <span className="text-metadata text-steel-500">{ti('dragDrop')}</span>
          <input
            ref={fileInput}
            type="file"
            accept="image/jpeg,image/png,image/webp,image/heic,image/heif,.heic,.heif"
            multiple
            hidden
            onChange={(e) => {
              if (e.target.files) void upload(e.target.files);
              e.target.value = '';
            }}
          />
        </div>
      )}

      {comparing && (
        <div className="rounded-lg bg-panel p-3 text-metadata text-steel-500">
          {pair.length < 2 ? t('comparePick') : null}
          {first && second && first.display_url && second.display_url && (
            <BeforeAfter
              before={first.display_url}
              after={second.display_url}
              beforeLabel={first.caption ?? lookupLabel(lookups?.image_categories, first.category)}
              afterLabel={second.caption ?? lookupLabel(lookups?.image_categories, second.category)}
            />
          )}
        </div>
      )}

      {query.isPending ? (
        <LoadingState />
      ) : query.isError ? (
        <ErrorState error={query.error} onRetry={() => void query.refetch()} />
      ) : shown.length === 0 ? (
        <p className="text-body text-steel-500">{ti('noImages')}</p>
      ) : (
        <ul className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
          {shown.map((img) => (
            <li key={img.id}>
              <button
                type="button"
                className={cn(
                  'group relative block w-full overflow-hidden rounded-lg border bg-panel text-left',
                  picked.includes(img.id) ? 'border-steel-900 ring-2 ring-steel-900' : 'border-steel-200',
                )}
                onClick={() => (comparing ? togglePick(img.id) : setOpen(all.indexOf(img)))}
              >
                {img.thumb_url ? (
                  <img src={img.thumb_url} alt={img.caption ?? img.original_filename ?? ''} className="aspect-[4/3] w-full object-cover" loading="lazy" />
                ) : (
                  <div className="flex aspect-[4/3] w-full items-center justify-center p-2 text-center text-metadata text-steel-500">
                    {img.processing_error ? t('noPreview') : t('processing')}
                  </div>
                )}
                <span className="absolute left-1 top-1 flex gap-1">
                  {img.immutable && (
                    <span className="rounded bg-steel-900/70 p-1 text-surface" title={t('evidence')}>
                      <Lock className="h-3 w-3" aria-label={t('evidence')} />
                    </span>
                  )}
                  {img.timestamp && (
                    <span className="rounded bg-done/90 p-1 text-surface" title={t('stamped')}>
                      <ShieldCheck className="h-3 w-3" aria-label={t('stamped')} />
                    </span>
                  )}
                  {img.annotated && (
                    <span className="rounded bg-signal/90 p-1 text-surface" title={t('annotated')}>
                      <PenLine className="h-3 w-3" aria-label={t('annotated')} />
                    </span>
                  )}
                </span>
                <span className="block truncate px-2 py-1 text-metadata text-steel-500">
                  {img.caption ?? lookupLabel(lookups?.image_categories, img.category)}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}

      {open !== null && all[open] && (
        <Lightbox
          images={all}
          index={open}
          onIndex={setOpen}
          onClose={() => setOpen(null)}
          orderId={orderId}
          vehicles={vehicles}
          canEdit={canUpload}
          canDelete={canDelete}
          canViewOriginal={canViewOriginal}
        />
      )}
    </section>
  );
}

/** The picture at `index`, or nothing when the list moved under the viewer. */
function Lightbox(props: {
  images: ImageView[];
  index: number;
  onIndex: (i: number) => void;
  onClose: () => void;
  orderId: number;
  vehicles: Vehicle[];
  canEdit: boolean;
  canDelete: boolean;
  canViewOriginal: boolean;
}) {
  const img = props.images[props.index];
  if (!img) return null;
  return <LightboxView {...props} img={img} />;
}

function LightboxView({
  img,
  images,
  index,
  onIndex,
  onClose,
  orderId,
  vehicles,
  canEdit,
  canDelete,
  canViewOriginal,
}: {
  img: ImageView;
  images: ImageView[];
  index: number;
  onIndex: (i: number) => void;
  onClose: () => void;
  orderId: number;
  vehicles: Vehicle[];
  canEdit: boolean;
  canDelete: boolean;
  canViewOriginal: boolean;
}) {
  const t = useTranslations('media');
  const ti = useTranslations('images');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const { toast } = useToast();
  const { data: lookups } = useLookups();
  const [drawing, setDrawing] = useState(false);
  const [size, setSize] = useState({ w: 0, h: 0 });
  const picture = useRef<HTMLImageElement>(null);

  useEffect(() => {
    const el = picture.current;
    if (!el) return;
    const measure = () => setSize({ w: el.clientWidth, h: el.clientHeight });
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [img.id]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (drawing) return;
      if (e.key === 'ArrowLeft' && index > 0) onIndex(index - 1);
      if (e.key === 'ArrowRight' && index < images.length - 1) onIndex(index + 1);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [drawing, index, images.length, onIndex]);

  const annotations = useQuery({
    queryKey: qk.annotations(img.id),
    queryFn: () => mediaApi.annotations(img.id),
    enabled: img.annotated || drawing,
  });
  const shapes: Shape[] = parseShapes(annotations.data?.shapes);

  const refresh = () => void qc.invalidateQueries({ queryKey: qk.images(orderId) });
  const patch = useMutation({
    mutationFn: (body: { caption?: string | null; vehicle_id?: number | null }) => mediaApi.updateImage(img.id, body),
    onSuccess: refresh,
    onError: (e) => toast('error', errorMessage(e, ter, ter('unknownError'))),
  });
  const save = useMutation({
    mutationFn: (next: Shape[]) => mediaApi.saveAnnotations(img.id, { shapes: next as unknown as Record<string, unknown>[] }),
    onSuccess: () => {
      setDrawing(false);
      void qc.invalidateQueries({ queryKey: qk.annotations(img.id) });
      refresh();
    },
    onError: (e) => toast('error', errorMessage(e, ter, ter('unknownError'))),
  });
  const remove = useMutation({
    mutationFn: () => mediaApi.deleteImage(img.id),
    onSuccess: () => {
      onClose();
      void qc.invalidateQueries({ queryKey: ['order', orderId] });
    },
    onError: (e) => toast('error', errorMessage(e, ter, ter('unknownError'))),
  });
  const openOriginal = async () => {
    try {
      const { url } = await mediaApi.original(img.id);
      window.open(url, '_blank', 'noopener');
    } catch (e) {
      toast('error', errorMessage(e, ter, ter('unknownError')));
    }
  };

  return (
    <Dialog.Root open onOpenChange={(o) => !o && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-steel-900/80" />
        <Dialog.Content className="fixed inset-2 z-50 flex flex-col overflow-hidden rounded-xl bg-surface lg:inset-6 lg:flex-row">
          <Dialog.Title className="sr-only">{img.caption ?? img.original_filename ?? t('photo')}</Dialog.Title>
          <div className="relative flex min-h-0 flex-1 items-center justify-center bg-steel-900 p-2">
            {img.display_url ? (
              <div className="relative inline-block max-h-full max-w-full">
                <img
                  ref={picture}
                  src={img.display_url}
                  alt={img.caption ?? ''}
                  className="block max-h-[calc(100vh-6rem)] max-w-full select-none object-contain"
                  draggable={false}
                />
                {drawing ? (
                  <AnnotationEditor
                    initial={shapes}
                    width={size.w}
                    height={size.h}
                    saving={save.isPending}
                    onSave={(next) => save.mutate(next)}
                    onCancel={() => setDrawing(false)}
                  />
                ) : (
                  <ShapesSvg shapes={shapes} width={size.w} height={size.h} />
                )}
              </div>
            ) : (
              <p className="text-body text-surface">{img.processing_error ?? t('processing')}</p>
            )}
            {!drawing && index > 0 && (
              <button type="button" className="absolute left-2 top-1/2 rounded-full bg-surface/80 p-2" onClick={() => onIndex(index - 1)} aria-label={ti('lightboxPrev')}>
                <ChevronLeft className="h-5 w-5" aria-hidden />
              </button>
            )}
            {!drawing && index < images.length - 1 && (
              <button type="button" className="absolute right-2 top-1/2 rounded-full bg-surface/80 p-2" onClick={() => onIndex(index + 1)} aria-label={ti('lightboxNext')}>
                <ChevronRight className="h-5 w-5" aria-hidden />
              </button>
            )}
          </div>
          <aside className="w-full shrink-0 space-y-4 overflow-y-auto border-steel-200 p-4 lg:w-80 lg:border-l">
            <div className="flex items-start justify-between gap-2">
              <div>
                <StatusBadge tone={img.immutable ? 'signal' : 'steel'}>
                  {lookupLabel(lookups?.image_categories, img.category)}
                </StatusBadge>
                <p className="mt-1 font-mono text-metadata text-steel-500">
                  {index + 1} / {images.length}
                </p>
              </div>
              <Dialog.Close className="btn-ghost btn-sm" aria-label={ti('lightboxClose')}>
                <X className="h-4 w-4" aria-hidden />
              </Dialog.Close>
            </div>

            <div>
              <label className="label" htmlFor={`caption-${img.id}`}>{t('caption')}</label>
              <textarea
                key={img.id}
                id={`caption-${img.id}`}
                rows={2}
                className="input"
                defaultValue={img.caption ?? ''}
                disabled={!canEdit}
                onBlur={(e) => {
                  const next = e.target.value.trim();
                  if (next !== (img.caption ?? '')) patch.mutate({ caption: next || null });
                }}
              />
            </div>

            {vehicles.length > 1 && (
              <div>
                <label className="label" htmlFor={`vehicle-${img.id}`}>{t('vehicle')}</label>
                <select
                  id={`vehicle-${img.id}`}
                  className="input"
                  value={img.vehicle_id ?? ''}
                  disabled={!canEdit}
                  onChange={(e) => patch.mutate({ vehicle_id: e.target.value ? Number(e.target.value) : null })}
                >
                  <option value="">—</option>
                  {vehicles.map((v) => (
                    <option key={v.id} value={v.id}>
                      {v.plate ?? v.vin ?? `#${v.id}`}
                    </option>
                  ))}
                </select>
              </div>
            )}

            <dl className="space-y-1 text-metadata">
              <div className="flex justify-between gap-2">
                <dt className="text-steel-500">{t('taken')}</dt>
                <dd>{img.captured_at ? <DateDisplay withTime value={img.captured_at} /> : '—'}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-steel-500">{t('uploadedAt')}</dt>
                <dd><DateDisplay withTime value={img.uploaded_at} /></dd>
              </div>
              {img.original_filename && (
                <div className="flex justify-between gap-2">
                  <dt className="text-steel-500">{t('file')}</dt>
                  <dd className="truncate font-mono">{img.original_filename}</dd>
                </div>
              )}
              <div className="flex justify-between gap-2">
                <dt className="text-steel-500">SHA-256</dt>
                <dd className="font-mono" title={img.content_hash}>{img.content_hash.slice(0, 16)}…</dd>
              </div>
            </dl>

            {img.timestamp && (
              <div className="rounded-lg bg-done/10 p-3 text-metadata">
                <p className="flex items-center gap-1 font-semibold text-done">
                  <ShieldCheck className="h-4 w-4" aria-hidden /> {t('stamped')}
                </p>
                <p className="mt-1">
                  <DateDisplay withTime value={img.timestamp.gen_time} />
                </p>
                <p className="truncate text-steel-500" title={img.timestamp.tsa_url}>{img.timestamp.tsa_url}</p>
                <a className="mt-1 inline-flex items-center gap-1 underline" href={mediaApi.timestampUrl(img.id)} download>
                  <Download className="h-3 w-3" aria-hidden /> {t('stampFile')}
                </a>
              </div>
            )}
            {img.immutable && <p className="text-metadata text-steel-500">{ti('immutable')}</p>}

            <div className="flex flex-wrap gap-2">
              {canEdit && img.display_url && !drawing && (
                <button type="button" className="btn-secondary btn-sm" onClick={() => setDrawing(true)}>
                  <PenLine className="h-4 w-4" aria-hidden /> {t('draw')}
                </button>
              )}
              {canViewOriginal && (
                <button type="button" className="btn-secondary btn-sm" onClick={() => void openOriginal()}>
                  <Download className="h-4 w-4" aria-hidden /> {ti('viewOriginal')}
                </button>
              )}
              {canDelete && !img.immutable && (
                <button
                  type="button"
                  className="btn-ghost btn-sm text-signal"
                  disabled={remove.isPending}
                  onClick={() => {
                    if (window.confirm(t('deleteConfirm'))) remove.mutate();
                  }}
                >
                  <Trash2 className="h-4 w-4" aria-hidden /> {tc('delete')}
                </button>
              )}
            </div>
          </aside>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
