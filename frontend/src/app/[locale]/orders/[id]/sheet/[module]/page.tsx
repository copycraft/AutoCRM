'use client';

// One department's technical sheet, ready to print: the job, the vehicle, that department's
// part of the technical content and the drawings and photos it uses — and nothing of the
// other departments', nor any price. `all` prints every part, each on its own page.
// Opened from the order's "Lap nyomtatása" in a new tab; `?print=1` prints at once, so it
// is one click from the order to the printer.

import { useEffect } from 'react';
import { useParams, useSearchParams } from 'next/navigation';
import { useTranslations } from 'next-intl';
import { format } from 'date-fns';
import { hu } from 'date-fns/locale';
import { useQuery } from '@tanstack/react-query';
import { mediaApi, modulesApi, ordersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { DateDisplay } from '@/components/ui/DateDisplay';
import { LoadingState } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import type { ModuleView, OrderDetail } from '@/lib/api/types';

/** Paper needs the date itself: "a perce" means nothing a week later on the workshop wall. */
function stamp(iso: string): string {
  return format(new Date(iso), 'yyyy. MM. dd. HH:mm', { locale: hu });
}

export default function ModuleSheetPage() {
  const t = useTranslations('modules');
  const params = useParams<{ id: string; module: string }>();
  const search = useSearchParams();
  const orderId = Number(params.id);
  const which = params.module;

  const detail = useQuery({ queryKey: qk.order(orderId), queryFn: () => ordersApi.get(orderId) });
  const modules = useQuery({ queryKey: ['order', orderId, 'modules'], queryFn: () => modulesApi.forOrder(orderId) });
  const images = useQuery({ queryKey: ['order', orderId, 'images', undefined], queryFn: () => mediaApi.images(orderId) });

  const chosen = (modules.data?.items ?? []).filter((m) => which === 'all' || String(m.id) === which);
  const ready = !!detail.data && !!modules.data && !!images.data;

  // Print once everything, photos included, is on the page.
  useEffect(() => {
    if (!ready || search.get('print') !== '1') return;
    const pending = Array.from(document.images).filter((img) => !img.complete);
    let left = pending.length;
    const go = () => window.setTimeout(() => window.print(), 150);
    if (left === 0) {
      go();
      return;
    }
    const done = () => {
      left -= 1;
      if (left === 0) go();
    };
    pending.forEach((img) => {
      img.addEventListener('load', done, { once: true });
      img.addEventListener('error', done, { once: true });
    });
  }, [ready, search]);

  if (detail.isError || modules.isError) {
    return <ErrorState error={detail.error ?? modules.error} onRetry={() => window.location.reload()} />;
  }
  if (!ready) return <LoadingState />;
  if (chosen.length === 0) return <p className="p-6 text-body">{t('sheetMissing')}</p>;

  return (
    <main className="mx-auto max-w-3xl bg-white p-6 text-steel-900 print:max-w-none print:p-0">
      <div className="mb-4 flex justify-end gap-2 print:hidden">
        <button type="button" className="btn-primary btn-sm" onClick={() => window.print()}>
          {t('printNow')}
        </button>
      </div>
      {chosen.map((m, i) => (
        <Sheet
          key={m.id}
          detail={detail.data!}
          module={m}
          imageUrls={Object.fromEntries((images.data?.items ?? []).map((img) => [img.id, img.display_url ?? img.thumb_url ?? '']))}
          breakBefore={i > 0}
        />
      ))}
    </main>
  );
}

function Sheet({
  detail,
  module: m,
  imageUrls,
  breakBefore,
}: {
  detail: OrderDetail;
  module: ModuleView;
  imageUrls: Record<number, string>;
  breakBefore: boolean;
}) {
  const t = useTranslations('modules');
  const { order, partner } = detail;
  const vehicle = [order.vehicle_make, order.vehicle_model].filter(Boolean).join(' ');
  const photos = m.files.filter((f) => f.image_id != null && imageUrls[f.image_id]);
  const docs = m.files.filter((f) => f.document_id != null);
  return (
    <article className={breakBefore ? 'mt-10 break-before-page print:mt-0' : ''}>
      <div className="h-3 w-full" style={{ backgroundColor: m.department_color }} />
      <header className="flex items-start justify-between border-b-2 border-steel-900 py-3">
        <div>
          <p className="text-metadata font-medium tracking-wide">
            AUTOTHERM · {t('sheetTitle')} ·{' '}
            <span style={{ color: m.department_color }} className="font-semibold">
              {m.department_name.toUpperCase()}
            </span>
          </p>
          <p className="text-section font-semibold">
            #{order.number} · {order.title}
          </p>
          <p className="text-body">{partner.name}</p>
        </div>
        <div className="text-right text-metadata">
          <p>
            {t('sheetPrinted')}: {stamp(new Date().toISOString())}
          </p>
          {order.due_date && (
            <p>
              {t('sheetDue')}: <DateDisplay value={order.due_date} />
            </p>
          )}
          {m.released_at && (
            <p>
              {t('releasedAt')}: {stamp(m.released_at)}
            </p>
          )}
        </div>
      </header>

      <dl className="grid grid-cols-2 gap-x-6 gap-y-1 py-3 text-body">
        <div>
          <dt className="text-metadata text-steel-500">{t('sheetVehicle')}</dt>
          <dd>{vehicle || '—'}</dd>
        </div>
        <div>
          <dt className="text-metadata text-steel-500">{t('sheetPlate')}</dt>
          <dd className="font-mono">{order.vehicle_plate ?? '—'}</dd>
        </div>
      </dl>

      <section className="border-t border-steel-200 py-3">
        <h2 className="text-body font-semibold">{t('sheetContent')}</h2>
        <p className="mt-2 whitespace-pre-wrap text-body leading-relaxed">{m.content || '—'}</p>
      </section>

      {docs.length > 0 && (
        <section className="border-t border-steel-200 py-3">
          <h2 className="text-body font-semibold">{t('sheetDrawings')}</h2>
          <ul className="mt-1 list-disc pl-5 text-body">
            {docs.map((f) => (
              <li key={f.id}>{f.filename}</li>
            ))}
          </ul>
        </section>
      )}

      {photos.length > 0 && (
        <section className="border-t border-steel-200 py-3">
          <h2 className="text-body font-semibold">{t('sheetPhotos')}</h2>
          <div className="mt-2 grid grid-cols-2 gap-3">
            {photos.map((f) => (
              // eslint-disable-next-line @next/next/no-img-element
              <img
                key={f.id}
                src={imageUrls[f.image_id!]}
                alt={f.filename ?? ''}
                className="w-full break-inside-avoid rounded border border-steel-200 object-contain"
              />
            ))}
          </div>
        </section>
      )}

      <footer className="mt-8 grid grid-cols-2 gap-10 text-metadata">
        <div className="border-t border-steel-900 pt-1">{t('sheetDoneBy')}</div>
        <div className="border-t border-steel-900 pt-1">{t('sheetChecked')}</div>
      </footer>
    </article>
  );
}
