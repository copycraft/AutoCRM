'use client';

// Intake slip (átvételi lap): what the vehicle was like when it arrived. Read-only once
// filled, inline form while empty. Leaving `intake` requires the mileage server-side, so
// this is also where the stage dialog's `intakeSlipMissing` error points.
//
// Mileage is the gate; fuel, keys and valuables are optional but printed next to it,
// because the arguments a month later are about a half-empty tank and a missing second
// key as often as they are about a scratch.

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ordersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { useLookups } from '@/hooks/useLookups';
import { errorMessage } from '@/lib/api/errors';
import type { Order } from '@/lib/api/types';

export function IntakeSlipSection({
  order,
  editable,
}: {
  order: Order;
  editable: boolean;
}) {
  const t = useTranslations('orders');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  // The marks on a fuel gauge, in gauge order, from the server (matches the
  // CHECK in 0019_intake_extras).
  const { data: lookups } = useLookups();
  const fuelLevels = lookups?.fuel_levels ?? [];
  const [editing, setEditing] = useState(false);
  const [mileage, setMileage] = useState(
    order.mileage_in != null ? String(order.mileage_in) : '',
  );
  const [condition, setCondition] = useState(order.intake_condition ?? '');
  const [fuel, setFuel] = useState<string | null>(order.fuel_level ?? null);
  const [keys, setKeys] = useState(order.key_count != null ? String(order.key_count) : '');
  // Three states, and the checkbox only covers two of them: unticked here means "asked,
  // nothing in the car", which is a different answer from never having filled the slip.
  const [hasValuables, setHasValuables] = useState(order.valuables_declared === true);
  const [valuables, setValuables] = useState(order.valuables ?? '');
  const [error, setError] = useState<string | null>(null);

  const save = useMutation({
    mutationFn: () => {
      const m = mileage.trim();
      const mileage_in = m === '' ? null : Number(m);
      if (mileage_in !== null && (!Number.isInteger(mileage_in) || mileage_in < 0)) {
        throw new Error('numeric');
      }
      const k = keys.trim();
      const key_count = k === '' ? null : Number(k);
      if (key_count !== null && (!Number.isInteger(key_count) || key_count < 0)) {
        throw new Error('keysNumeric');
      }
      return ordersApi.patch(order.id, {
        mileage_in,
        intake_condition: condition.trim() ? condition.trim() : null,
        fuel_level: fuel,
        key_count,
        // Saving the slip at all is what records the answer, so the box is always a
        // definite true/false from here — never null, which would mean "not asked".
        valuables_declared: hasValuables,
        valuables: hasValuables && valuables.trim() ? valuables.trim() : null,
      });
    },
    onSuccess: () => {
      setEditing(false);
      setError(null);
      void qc.invalidateQueries({ queryKey: qk.order(order.id) });
      void qc.invalidateQueries({ queryKey: ['orders'] });
    },
    onError: (e) =>
      setError(
        e instanceof Error && e.message === 'numeric'
          ? t('intakeMileageNumeric')
          : e instanceof Error && e.message === 'keysNumeric'
            ? t('intakeKeysNumeric')
            : errorMessage(e, ter, ter('unknownError')),
      ),
  });

  const filled =
    order.mileage_in != null ||
    order.intake_condition != null ||
    order.fuel_level != null ||
    order.key_count != null ||
    order.valuables_declared != null;

  return (
    <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0">
      <div className="flex items-center justify-between">
        <h2 className="text-section font-semibold">{t('intakeTitle')}</h2>
        {editable && !editing && (
          <button className="btn-secondary btn-sm" onClick={() => setEditing(true)}>
            {filled ? tc('edit') : t('intakeRecord')}
          </button>
        )}
      </div>
      {editing ? (
        <div className="mt-3 grid grid-cols-1 gap-4 md:grid-cols-2">
          <div>
            <label className="label" htmlFor="slip-mileage">
              {t('intakeMileage')} *
            </label>
            <input
              id="slip-mileage"
              inputMode="numeric"
              className="input font-mono"
              value={mileage}
              onChange={(e) => setMileage(e.target.value)}
            />
          </div>
          <div>
            <label className="label" htmlFor="slip-keys">
              {t('intakeKeys')}
            </label>
            <input
              id="slip-keys"
              inputMode="numeric"
              className="input font-mono"
              value={keys}
              onChange={(e) => setKeys(e.target.value)}
            />
          </div>
          <div>
            <span className="label">{t('intakeFuel')}</span>
            {/* A gauge, not a text field: five marks is the whole vocabulary, and typing
                it invites "1/2 tank" and "fél" in the same column. */}
            <div className="mt-1 flex gap-1" role="group" aria-label={t('intakeFuel')}>
              {fuelLevels.map((level) => (
                <button
                  key={level.key}
                  type="button"
                  aria-pressed={fuel === level.key}
                  className={
                    fuel === level.key
                      ? 'btn-primary btn-sm flex-1 font-mono'
                      : 'btn-secondary btn-sm flex-1 font-mono'
                  }
                  onClick={() => setFuel(fuel === level.key ? null : level.key)}
                >
                  {level.label_hu}
                </button>
              ))}
            </div>
          </div>
          <div>
            <label className="label" htmlFor="slip-condition">
              {t('intakeCondition')}
            </label>
            <textarea
              id="slip-condition"
              rows={2}
              className="input"
              value={condition}
              onChange={(e) => setCondition(e.target.value)}
            />
          </div>
          <div className="md:col-span-2">
            <label className="flex items-center gap-2 text-body">
              <input
                type="checkbox"
                checked={hasValuables}
                onChange={(e) => setHasValuables(e.target.checked)}
              />
              {t('intakeValuablesPresent')}
            </label>
            {hasValuables && (
              <input
                className="input mt-2"
                placeholder={t('intakeValuablesPlaceholder')}
                value={valuables}
                onChange={(e) => setValuables(e.target.value)}
              />
            )}
          </div>
          {error && (
            <p className="text-body text-steel-900 md:col-span-2" role="alert">
              {error}
            </p>
          )}
          <div className="flex gap-2 md:col-span-2">
            <button
              className="btn-primary btn-sm"
              disabled={save.isPending || !mileage.trim()}
              onClick={() => save.mutate()}
            >
              {save.isPending ? tc('saving') : tc('save')}
            </button>
            <button className="btn-ghost btn-sm" onClick={() => setEditing(false)}>
              {tc('cancel')}
            </button>
          </div>
        </div>
      ) : filled ? (
        <div className="mt-3 grid grid-cols-1 gap-4 sm:grid-cols-2">
          <p className="text-body">
            <span className="text-metadata text-steel-500">{t('intakeMileage')}: </span>
            <span className="font-mono">
              {order.mileage_in != null ? `${order.mileage_in.toLocaleString('hu-HU')} km` : '—'}
            </span>
          </p>
          <p className="text-body">
            <span className="text-metadata text-steel-500">{t('intakeFuel')}: </span>
            <span className="font-mono">{order.fuel_level ?? '—'}</span>
          </p>
          <p className="text-body">
            <span className="text-metadata text-steel-500">{t('intakeKeys')}: </span>
            <span className="font-mono">{order.key_count ?? '—'}</span>
          </p>
          <p className="text-body">
            <span className="text-metadata text-steel-500">{t('intakeValuables')}: </span>
            {order.valuables_declared == null
              ? '—'
              : order.valuables_declared
                ? (order.valuables ?? t('intakeValuablesUnlisted'))
                : t('intakeValuablesNone')}
          </p>
          <p className="text-body sm:col-span-2">
            <span className="text-metadata text-steel-500">{t('intakeCondition')}: </span>
            {order.intake_condition ?? '—'}
          </p>
        </div>
      ) : (
        <p className="mt-2 text-body text-steel-500">{t('intakeEmpty')}</p>
      )}
    </section>
  );
}
