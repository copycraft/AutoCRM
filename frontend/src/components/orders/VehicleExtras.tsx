'use client';

// Small vehicle helpers (0049): reading a VIN (maker, region, model year — and make and
// model when the online lookup is on), and the cooling unit's serial number on the spec.

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ScanSearch } from 'lucide-react';
import { orderExtrasApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import type { components } from '@/lib/api/schema.gen';

type VinInfo = components['schemas']['VinInfo'];

/** "Read" next to the VIN field: shows what it says and offers to fill make and model. */
export function VinDecodeButton({
  vin,
  onFill,
}: {
  vin: string;
  onFill: (fields: { make?: string; model?: string }) => void;
}) {
  const t = useTranslations('vin');
  const ter = useTranslations('errors');
  const [info, setInfo] = useState<VinInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const decode = useMutation({
    mutationFn: () => orderExtrasApi.decodeVin(vin),
    onSuccess: (r) => {
      setError(null);
      setInfo(r);
    },
    onError: (e) => {
      setInfo(null);
      setError(errorMessage(e, ter, ter('unknownError')));
    },
  });
  const make = info?.make ?? info?.manufacturer?.replace(/\s*\(.*\)$/, '') ?? undefined;
  return (
    <div className="mt-1 space-y-1">
      <button
        type="button"
        className="inline-flex items-center gap-1 text-metadata underline disabled:opacity-50"
        disabled={vin.trim().length < 17 || decode.isPending}
        onClick={() => decode.mutate()}
      >
        <ScanSearch className="h-3.5 w-3.5" aria-hidden />
        {t('decode')}
      </button>
      {error && <p className="text-metadata text-signal">{error}</p>}
      {info && (
        <div className="rounded bg-steel-100 px-2 py-1 text-metadata text-steel-700">
          {[
            info.manufacturer ?? info.make,
            info.model,
            info.model_year ? t('modelYear', { year: info.model_year }) : null,
            info.region,
          ]
            .filter(Boolean)
            .join(' · ') || t('unknown')}
          {!info.check_digit_ok && info.region === 'Észak-Amerika' && (
            <span className="ml-1 text-signal">{t('checkDigit')}</span>
          )}
          {make && (
            <button
              type="button"
              className="ml-2 underline"
              onClick={() => onFill({ make, model: info.model ?? undefined })}
            >
              {t('fill')}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

/** The cooling unit's serial on the order page; scanned on the phone, editable here. */
export function CoolingSerial({ orderId, value, editable }: { orderId: number; value: string | null | undefined; editable: boolean }) {
  const t = useTranslations('vin');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value ?? '');
  const [error, setError] = useState<string | null>(null);
  const save = useMutation({
    mutationFn: () => orderExtrasApi.setCoolingSerial(orderId, draft.trim() || null),
    onSuccess: () => {
      setEditing(false);
      setError(null);
      void qc.invalidateQueries({ queryKey: ['order', orderId] });
      void qc.invalidateQueries({ queryKey: ['orders'] });
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });
  return (
    <div className="flex flex-col gap-0.5">
      <span className="text-metadata font-medium text-steel-500">{t('coolingSerial')}</span>
      {editing ? (
        <span className="flex items-center gap-2">
          <input
            className="input h-8 py-0 font-mono"
            value={draft}
            maxLength={100}
            autoFocus
            onChange={(e) => setDraft(e.target.value)}
            aria-label={t('coolingSerial')}
          />
          <button className="btn-primary btn-sm" disabled={save.isPending} onClick={() => save.mutate()}>
            {t('save')}
          </button>
        </span>
      ) : (
        <span className="font-mono text-body">
          {value || '—'}
          {editable && (
            <button type="button" className="ml-2 text-metadata underline" onClick={() => setEditing(true)}>
              {t('edit')}
            </button>
          )}
        </span>
      )}
      {error && <span className="text-metadata text-signal">{error}</span>}
    </div>
  );
}
