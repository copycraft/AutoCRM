'use client';

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useForm } from 'react-hook-form';
import { z } from 'zod';
import { zodResolver } from '@hookform/resolvers/zod';
import { ordersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { Money } from '@/components/ui/Money';
import { ConfirmDialog } from '@/components/ui/ConfirmDialog';
import { minorToMajorString, parseMajorToMinor } from '@/lib/utils/format';
import type { AddItem, Currency, ItemView, PatchItem } from '@/lib/api/types';

const schema = z.object({
  description: z.string().trim().min(1),
  quantity: z.string().trim().min(1),
  unit_price: z.string().trim().min(1),
});

type Values = z.infer<typeof schema>;

/** Display symbol for the order currency. Symbols, not words — no catalogue entry. */
function currencySymbol(currency: Currency): string {
  return currency === 'EUR' ? '€' : 'Ft';
}

function ItemForm({
  initial,
  currency,
  onSubmit,
  submitLabel,
  onCancel,
}: {
  initial?: ItemView;
  currency: Currency;
  onSubmit: (item: AddItem) => Promise<void>;
  submitLabel: string;
  onCancel: () => void;
}) {
  const t = useTranslations('orders');
  const tc = useTranslations('common');
  const tv = useTranslations('validation');
  const ter = useTranslations('errors');
  const [serverError, setServerError] = useState<string | null>(null);
  const { register, handleSubmit, formState } = useForm<Values>({
    resolver: zodResolver(schema),
    defaultValues: {
      description: initial?.description ?? '',
      quantity: initial?.quantity ?? '1',
      unit_price: initial ? minorToMajorString(initial.unit_price) : '',
    },
  });

  return (
    <form
      className="rounded-lg border border-steel-200 bg-panel p-4"
      noValidate
      onSubmit={handleSubmit(async (v) => {
        setServerError(null);
        // Major units in the user's own notation → minor units, exactly.
        // Negatives allowed: the backend permits discount lines.
        const unit = parseMajorToMinor(v.unit_price);
        if (unit === null) {
          setServerError(tv('numeric'));
          return;
        }
        // Quantity stays a decimal string for the backend ("2.5", never float).
        if (!/^\d+([.,]\d+)?$/.test(v.quantity.trim())) {
          setServerError(tv('numeric'));
          return;
        }
        try {
          await onSubmit({
            description: v.description.trim(),
            quantity: v.quantity.trim().replace(',', '.'),
            unit_price: unit,
          });
        } catch (e) {
          setServerError(errorMessage(e, ter, ter('unknownError')));
        }
      })}
    >
      <div className="grid grid-cols-1 gap-3 md:grid-cols-4">
        <div className="md:col-span-2">
          <label className="label" htmlFor="it-desc">{t('description')} *</label>
          <input id="it-desc" className="input" {...register('description')} />
          {formState.errors.description && <p className="mt-1 text-xs text-steel-900">{tv('required')}</p>}
        </div>
        <div>
          <label className="label" htmlFor="it-qty">{t('quantity')} *</label>
          <input id="it-qty" className="input font-mono" inputMode="decimal" placeholder="2.5" {...register('quantity')} />
        </div>
        <div>
          <label className="label" htmlFor="it-price">{t('unitPrice')} ({currencySymbol(currency)}) *</label>
          <input
            id="it-price"
            className="input font-mono"
            inputMode="decimal"
            placeholder={currency === 'EUR' ? '12 400,50' : '4 850 000'}
            {...register('unit_price')}
          />
        </div>
      </div>
      <p className="mt-2 text-xs text-steel-500">{t('currencySharedNote')}</p>
      {serverError && (
        <p className="mt-2 rounded-lg bg-steel-200/50 px-3 py-2 text-sm text-steel-900" role="alert">{serverError}</p>
      )}
      <div className="mt-3 flex justify-end gap-2">
        <button type="button" className="btn-ghost btn-sm" onClick={onCancel}>
          {tc('cancel')}
        </button>
        <button type="submit" className="btn-primary btn-sm" disabled={formState.isSubmitting}>
          {formState.isSubmitting ? tc('saving') : submitLabel}
        </button>
      </div>
    </form>
  );
}

/** PATCH diff: only fields that changed. */
function itemPatch(original: ItemView, v: AddItem): PatchItem {
  const body: PatchItem = {};
  if (v.description !== original.description) body.description = v.description;
  if (v.quantity !== original.quantity) body.quantity = v.quantity;
  if (v.unit_price !== original.unit_price) body.unit_price = v.unit_price;
  return body;
}

export function ItemsSection({
  orderId,
  items,
  currency,
}: {
  orderId: number;
  items: ItemView[];
  currency: Currency;
}) {
  const t = useTranslations('orders');
  const tc = useTranslations('common');
  const qc = useQueryClient();
  const [adding, setAdding] = useState(false);
  const [editing, setEditing] = useState<ItemView | null>(null);
  const [deleting, setDeleting] = useState<ItemView | null>(null);

  const invalidate = () => {
    void qc.invalidateQueries({ queryKey: qk.order(orderId) });
    void qc.invalidateQueries({ queryKey: ['orders'] });
  };

  const create = useMutation({
    mutationFn: (body: AddItem) => ordersApi.createItem(orderId, body),
    onSuccess: () => {
      setAdding(false);
      invalidate();
    },
  });
  const patch = useMutation({
    mutationFn: ({ id, body }: { id: number; body: PatchItem }) => ordersApi.patchItem(id, body),
    onSuccess: () => {
      setEditing(null);
      invalidate();
    },
  });
  const remove = useMutation({
    mutationFn: (id: number) => ordersApi.deleteItem(id),
    onSuccess: () => {
      setDeleting(null);
      invalidate();
    },
  });

  return (
    <div className="space-y-3">
      <div className="flex justify-end">
        {!adding && (
          <button className="btn-secondary btn-sm" onClick={() => setAdding(true)}>
            {t('addItem')}
          </button>
        )}
      </div>
      {adding && (
        <ItemForm
          currency={currency}
          submitLabel={tc('create')}
          onCancel={() => setAdding(false)}
          onSubmit={(b) => create.mutateAsync(b).then(() => undefined)}
        />
      )}
      <div className="table-container">
        <table className="table">
          <thead>
            <tr>
              <th scope="col">{t('positionCol')}</th>
              <th scope="col">{t('description')}</th>
              <th scope="col" className="text-right">{t('quantity')}</th>
              <th scope="col" className="text-right">{t('unitPrice')}</th>
              <th scope="col" className="text-right">{t('lineTotal')}</th>
              <th scope="col"><span className="sr-only">{tc('actions')}</span></th>
            </tr>
          </thead>
          <tbody>
            {items.map((it) =>
              editing?.id === it.id ? (
                <tr key={it.id}>
                  <td colSpan={6}>
                    <ItemForm
                      initial={it}
                      currency={currency}
                      submitLabel={t('saveItem')}
                      onCancel={() => setEditing(null)}
                      onSubmit={(v) => patch.mutateAsync({ id: it.id, body: itemPatch(it, v) }).then(() => undefined)}
                    />
                  </td>
                </tr>
              ) : (
                <tr key={it.id}>
                  <td className="font-mono text-metadata">{it.position}</td>
                  <td className="font-medium">{it.description}</td>
                  <td className="text-right font-mono">{it.quantity}</td>
                  <td className="text-right">
                    <Money minor={it.unit_price} currency={currency} />
                  </td>
                  <td className="text-right">
                    <Money minor={it.line_total_minor} currency={currency} />
                  </td>
                  <td className="text-right whitespace-nowrap">
                    <button className="btn-ghost btn-sm" onClick={() => setEditing(it)}>
                      {tc('edit')}
                    </button>
                    <button className="btn-ghost btn-sm" onClick={() => setDeleting(it)}>
                      {tc('delete')}
                    </button>
                  </td>
                </tr>
              ),
            )}
            {items.length === 0 && !adding && (
              <tr>
                <td colSpan={6} className="text-center text-steel-500">
                  {t('noItems')}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      <ConfirmDialog
        open={deleting !== null}
        title={`${t('deleteItem')}: ${deleting?.description ?? ''}`}
        body={t('deleteItemBody')}
        confirmLabel={tc('delete')}
        onClose={() => setDeleting(null)}
        busy={remove.isPending}
        onConfirm={() => deleting && remove.mutate(deleting.id)}
      />
    </div>
  );
}
