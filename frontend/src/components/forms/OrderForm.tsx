'use client';

import { useState } from 'react';
import { useForm, Controller } from 'react-hook-form';
import { z } from 'zod';
import { zodResolver } from '@hookform/resolvers/zod';
import { useTranslations } from 'next-intl';
import { skipToken, useQuery } from '@tanstack/react-query';
import { errorMessage } from '@/lib/api/errors';
import { configApi, partnersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { PartnerPicker, type PartnerOption } from './PartnerPicker';
import { AssigneeField } from './AssigneeField';
import type { Order, OrderBody, PatchOrder } from '@/lib/api/types';

const schema = z.object({
  title: z.string().trim().min(1),
  // Partner is required, but enforced in the submit handler (not via
  // .refine) so the field type stays a plain nullable the form can handle.
  partner: z.custom<PartnerOption | null>(() => true),
  contact_id: z.string(),
  project_type_id: z.string(),
  currency: z.enum(['HUF', 'EUR']),
  valuation_date: z.string(),
  vehicle_make: z.string().trim().optional(),
  vehicle_model: z.string().trim().optional(),
  vehicle_plate: z.string().trim().optional(),
  vehicle_vin: z.string().trim().optional(),
  description: z.string().trim().optional(),
  due_date: z.string(),
  assigned_to: z.custom<number | null | 'me'>(() => true),
});

export type OrderFormValues = z.infer<typeof schema>;

function todayLocal(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

function assignee(v: OrderFormValues['assigned_to'], meId: number | undefined): number | null {
  return v === 'me' ? (meId ?? null) : v;
}

export function orderCreateBody(v: OrderFormValues, meId: number | undefined): OrderBody {
  const clean = (s: string | undefined) => (s?.trim() ? s.trim() : undefined);
  const date = (s: string) => (s ? s : undefined);
  return {
    title: v.title.trim(),
    // The form requires a partner; the backend rejects a missing partner_id regardless.
    partner_id: v.partner?.id,
    contact_id: v.contact_id ? Number(v.contact_id) : undefined,
    project_type_id: v.project_type_id ? Number(v.project_type_id) : undefined,
    currency: v.currency,
    valuation_date: date(v.valuation_date),
    vehicle_make: clean(v.vehicle_make),
    vehicle_model: clean(v.vehicle_model),
    vehicle_plate: clean(v.vehicle_plate),
    vehicle_vin: clean(v.vehicle_vin),
    description: clean(v.description),
    due_date: date(v.due_date),
    assigned_to: assignee(v.assigned_to, meId),
    items: [],
  };
}

/**
 * PATCH diff with backend semantics. Currency is never sent here — the
 * caller disables it while items exist (422 currency_locked otherwise).
 * Changing partner without explicitly picking a contact clears the contact,
 * mirroring the backend rule.
 */
export function orderPatchBody(
  original: Order,
  v: OrderFormValues,
  meId: number | undefined,
  contactDirty: boolean,
): PatchOrder {
  const body: PatchOrder = {};
  if (v.title.trim() !== original.title) body.title = v.title.trim();
  const newPartner = v.partner?.id ?? null;
  const partnerChanged = newPartner !== original.partner_id;
  if (partnerChanged && newPartner !== null) body.partner_id = newPartner;
  const newContact = v.contact_id ? Number(v.contact_id) : null;
  if (newContact !== (original.contact_id ?? null)) body.contact_id = newContact;
  else if (partnerChanged && contactDirty === false && original.contact_id != null) {
    body.contact_id = null;
  }
  const newPt = v.project_type_id ? Number(v.project_type_id) : null;
  if (newPt !== (original.project_type_id ?? null)) body.project_type_id = newPt;
  if (v.valuation_date && v.valuation_date !== original.valuation_date) {
    body.valuation_date = v.valuation_date;
  }
  for (const f of ['vehicle_make', 'vehicle_model', 'vehicle_plate', 'vehicle_vin', 'description'] as const) {
    const nv = v[f]?.trim() ?? '';
    if (nv !== (original[f] ?? '')) body[f] = nv ? nv : null;
  }
  const newDue = v.due_date ? v.due_date : null;
  if (newDue !== (original.due_date ?? null)) body.due_date = newDue;
  const assigned = assignee(v.assigned_to, meId);
  if (assigned !== (original.assigned_to ?? null)) body.assigned_to = assigned;
  return body;
}

export function OrderForm({
  initial,
  initialPartner,
  currencyLocked,
  onSubmit,
  submitLabel,
}: {
  initial?: Order;
  initialPartner?: PartnerOption | null;
  /** True while the order has items — currency select disabled. */
  currencyLocked?: boolean;
  onSubmit: (v: OrderFormValues, contactDirty: boolean) => Promise<void>;
  submitLabel: string;
}) {
  const t = useTranslations('orders');
  const tc = useTranslations('common');
  const tv = useTranslations('validation');
  const [serverError, setServerError] = useState<string | null>(null);
  const [partnerError, setPartnerError] = useState(false);

  const { register, handleSubmit, control, watch, formState } = useForm<OrderFormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      title: initial?.title ?? '',
      partner: initialPartner ?? null,
      contact_id: initial?.contact_id ? String(initial.contact_id) : '',
      project_type_id: initial?.project_type_id ? String(initial.project_type_id) : '',
      currency: initial?.currency ?? 'HUF',
      valuation_date: initial?.valuation_date ?? todayLocal(),
      vehicle_make: initial?.vehicle_make ?? '',
      vehicle_model: initial?.vehicle_model ?? '',
      vehicle_plate: initial?.vehicle_plate ?? '',
      vehicle_vin: initial?.vehicle_vin ?? '',
      description: initial?.description ?? '',
      due_date: initial?.due_date ?? '',
      assigned_to: initial?.assigned_to ?? null,
    },
  });

  const partner = watch('partner');
  const partnerId = partner?.id;
  const contactsQuery = useQuery({
    queryKey: qk.partner(partnerId ?? 0),
    queryFn: partnerId === undefined ? skipToken : () => partnersApi.get(partnerId),
  });
  const projectTypes = useQuery({
    queryKey: qk.projectTypes,
    queryFn: () => configApi.projectTypes(),
  });

  return (
    <form
      className="card"
      noValidate
      onSubmit={handleSubmit(async (v) => {
        setServerError(null);
        if (!v.partner) {
          setPartnerError(true);
          return;
        }
        try {
          await onSubmit(v, !!formState.dirtyFields.contact_id);
        } catch (e) {
          setServerError(errorMessage(e, 'Ismeretlen hiba történt.'));
        }
      })}
    >
      {serverError && (
        <div className="card-content pb-0">
          <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-sm text-steel-900" role="alert">{serverError}</p>
        </div>
      )}
      <div className="card-content grid grid-cols-1 gap-4 md:grid-cols-2">
        <div className="md:col-span-2">
          <label className="label" htmlFor="of-title">{t('fieldTitle')} *</label>
          <input id="of-title" className="input" {...register('title')} />
          {formState.errors.title && <p className="mt-1 text-xs text-steel-900">{tv('required')}</p>}
        </div>
        <Controller
          control={control}
          name="partner"
          render={({ field }) => (
            <div>
              <PartnerPicker
                value={field.value}
                onChange={(p) => {
                  field.onChange(p);
                  if (p) setPartnerError(false);
                }}
                label={`${tc('partner')} *`}
              />
              {partnerError && <p className="mt-1 text-xs text-steel-900">{tv('required')}</p>}
            </div>
          )}
        />
        <div>
          <label className="label" htmlFor="of-currency">{t('currencyLabel')} *</label>
          <select id="of-currency" className="input" {...register('currency')} disabled={currencyLocked}>
            <option value="HUF">HUF</option>
            <option value="EUR">EUR</option>
          </select>
          {currencyLocked && <p className="mt-1 text-xs text-steel-500">{t('currencyLocked')}</p>}
        </div>
        <div>
          <label className="label" htmlFor="of-contact">{tc('contact')}</label>
          <select id="of-contact" className="input" {...register('contact_id')} disabled={!partner}>
            <option value="">—</option>
            {contactsQuery.data?.contacts
              .filter((c) => !c.archived_at)
              .map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
          </select>
          {initial && <p className="mt-1 text-xs text-steel-500">{t('contactClearedNote')}</p>}
        </div>
        <div>
          <label className="label" htmlFor="of-pt">{t('projectType')}</label>
          <select id="of-pt" className="input" {...register('project_type_id')}>
            <option value="">—</option>
            {(projectTypes.data?.items ?? [])
              .filter((p) => p.is_active)
              .map((p) => (
                <option key={p.id} value={p.id}>
                  {p.label_hu}
                </option>
              ))}
          </select>
        </div>
        <div>
          <label className="label" htmlFor="of-val">{t('valuationDate')}</label>
          <input id="of-val" type="date" className="input font-mono" {...register('valuation_date')} />
        </div>
        <div>
          <label className="label" htmlFor="of-due">{t('dueDate')}</label>
          <input id="of-due" type="date" className="input font-mono" {...register('due_date')} />
        </div>
        <div>
          <label className="label" htmlFor="of-make">{t('vehicleMake')}</label>
          <input id="of-make" className="input" {...register('vehicle_make')} />
        </div>
        <div>
          <label className="label" htmlFor="of-model">{t('vehicleModel')}</label>
          <input id="of-model" className="input" {...register('vehicle_model')} />
        </div>
        <div>
          <label className="label" htmlFor="of-plate">{t('vehiclePlate')}</label>
          <input id="of-plate" className="input font-mono" {...register('vehicle_plate')} />
        </div>
        <div>
          <label className="label" htmlFor="of-vin">{t('vehicleVin')}</label>
          <input id="of-vin" className="input font-mono" {...register('vehicle_vin')} />
        </div>
        <div className="md:col-span-2">
          <label className="label" htmlFor="of-desc">{t('description')}</label>
          <textarea id="of-desc" rows={3} className="input" {...register('description')} />
        </div>
        <Controller
          control={control}
          name="assigned_to"
          render={({ field }) => (
            <AssigneeField label={t('assignedTo')} value={field.value} onChange={field.onChange} />
          )}
        />
      </div>
      <div className="card-footer justify-end">
        <button className="btn-primary" type="submit" disabled={formState.isSubmitting}>
          {formState.isSubmitting ? 'Mentés…' : submitLabel}
        </button>
      </div>
    </form>
  );
}
