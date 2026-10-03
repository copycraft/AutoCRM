'use client';

import { useState } from 'react';
import { useForm } from 'react-hook-form';
import { z } from 'zod';
import { zodResolver } from '@hookform/resolvers/zod';
import { useTranslations } from 'next-intl';
import { errorMessage } from '@/lib/api/errors';
import { useLookups } from '@/hooks/useLookups';
import { useDirtyGuard } from '@/hooks/useDirtyGuard';
import { useFormDraft } from '@/hooks/useFormDraft';
import type { CreatePartner, Partner, PartnerKind, PatchPartner } from '@/lib/api/types';

const schema = z.object({
  kind: z.enum(['business', 'person']),
  name: z.string().trim().min(1),
  tax_number: z.string().trim().optional(),
  eu_tax_number: z.string().trim().optional(),
  country: z.string().trim().min(2).max(2),
  // The server owns the currency list (`GET /config/lookups`) and validates;
  // the form only requires something chosen.
  default_currency: z.string().min(1),
  email: z.string().trim().optional(),
  phone: z.string().trim().optional(),
  website: z.string().trim().optional(),
  postal_code: z.string().trim().optional(),
  city: z.string().trim().optional(),
  address_line: z.string().trim().optional(),
  notes: z.string().trim().optional(),
});

export type PartnerFormValues = z.infer<typeof schema>;

function toForm(p?: Partner, initialKind?: PartnerKind): PartnerFormValues {
  return {
    kind: p?.kind ?? initialKind ?? 'business',
    name: p?.name ?? '',
    tax_number: p?.tax_number ?? '',
    eu_tax_number: p?.eu_tax_number ?? '',
    country: p?.country ?? 'HU',
    default_currency: p?.default_currency ?? 'HUF',
    email: p?.email ?? '',
    phone: p?.phone ?? '',
    website: p?.website ?? '',
    postal_code: p?.postal_code ?? '',
    city: p?.city ?? '',
    address_line: p?.address_line ?? '',
    notes: p?.notes ?? '',
  };
}

/**
 * Build a PATCH body with backend semantics: omit unchanged, null clears.
 * Text fields: '' on a previously-filled value → null (backend blank→null).
 */
export function partnerPatchBody(original: Partner, v: PartnerFormValues): PatchPartner {
  const body: PatchPartner = {};
  if (v.kind !== original.kind) body.kind = v.kind;
  if (v.name.trim() !== original.name) body.name = v.name.trim();
  if ((v.tax_number?.trim() ?? '') !== (original.tax_number ?? '')) {
    body.tax_number = v.tax_number?.trim() ? v.tax_number.trim() : null;
  }
  if ((v.eu_tax_number?.trim() ?? '') !== (original.eu_tax_number ?? '')) {
    body.eu_tax_number = v.eu_tax_number?.trim() ? v.eu_tax_number.trim() : null;
  }
  if (v.country.trim().toUpperCase() !== original.country) {
    body.country = v.country.trim().toUpperCase();
  }
  if (v.default_currency !== original.default_currency) {
    body.default_currency = v.default_currency as Partner['default_currency'];
  }
  for (const f of ['email', 'phone', 'website', 'postal_code', 'city', 'address_line', 'notes'] as const) {
    const nv = v[f]?.trim() ?? '';
    if (nv !== (original[f] ?? '')) body[f] = nv ? nv : null;
  }
  return body;
}

export function partnerCreateBody(v: PartnerFormValues): CreatePartner {
  const clean = (s: string | undefined) => (s?.trim() ? s.trim() : undefined);
  return {
    kind: v.kind,
    name: v.name.trim(),
    tax_number: clean(v.tax_number),
    eu_tax_number: clean(v.eu_tax_number),
    country: v.country.trim().toUpperCase() || 'HU',
    default_currency: v.default_currency as CreatePartner['default_currency'],
    email: clean(v.email),
    phone: clean(v.phone),
    website: clean(v.website),
    postal_code: clean(v.postal_code),
    city: clean(v.city),
    address_line: clean(v.address_line),
    notes: clean(v.notes),
  };
}

export function PartnerForm({
  initial,
  initialKind,
  draftKey,
  onSubmit,
  submitLabel,
}: {
  initial?: Partner;
  /** Preset kind for create (from the menu the user came from); still changeable. */
  initialKind?: PartnerKind;
  /** Draft autosave slot for the create page; ignored when editing. */
  draftKey?: string;
  onSubmit: (v: PartnerFormValues) => Promise<void>;
  submitLabel: string;
}) {
  const t = useTranslations('partners');
  const tc = useTranslations('common');
  const tq = useTranslations('qol');
  const tv = useTranslations('validation');
  const ter = useTranslations('errors');
  const [serverError, setServerError] = useState<string | null>(null);
  const { data: lookups } = useLookups();
  const currencies = lookups?.currencies ?? [];
  const emptyPartner = toForm(undefined, initialKind);
  const {
    register,
    handleSubmit,
    watch,
    reset,
    formState: { errors, isSubmitting, isDirty, isSubmitSuccessful },
  } = useForm<PartnerFormValues>({ resolver: zodResolver(schema), defaultValues: toForm(initial, initialKind) });
  const draft = useFormDraft({
    key: draftKey && !initial ? draftKey : null,
    watch,
    reset,
    empty: emptyPartner,
  });
  useDirtyGuard(isDirty && !isSubmitSuccessful, tq('unsavedChanges'));

  return (
    <form
      className="card"
      noValidate
      onSubmit={handleSubmit(async (v) => {
        setServerError(null);
        try {
          await onSubmit(v);
          draft.clear();
        } catch (e) {
          setServerError(errorMessage(e, ter, ter('unknownError')));
        }
      })}
    >
      {draft.restored && (
        <div className="card-content pb-0">
          <p className="flex flex-wrap items-center gap-2 rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900">
            <span>{tq('draftRestored')}</span>
            <button type="button" className="btn-ghost btn-sm" onClick={draft.discard}>
              {tq('draftDiscard')}
            </button>
          </p>
        </div>
      )}
      {serverError && (
        <div className="card-content pb-0">
          <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">
            {serverError}
          </p>
        </div>
      )}
      <div className="card-content grid grid-cols-1 gap-4 md:grid-cols-2">
        <div>
          <label className="label" htmlFor="kind">{t('kindLabel')}</label>
          <select id="kind" className="input" {...register('kind')}>
            <option value="business">{t('business')}</option>
            <option value="person">{t('person')}</option>
          </select>
        </div>
        <div>
          <label className="label" htmlFor="name">{tc('name')} *</label>
          <input id="name" className="input" {...register('name')} />
          {errors.name && <p className="mt-1 text-metadata text-steel-900">{tv('required')}</p>}
        </div>
        <div>
          <label className="label" htmlFor="tax_number">{t('taxNumber')}</label>
          <input id="tax_number" className="input font-mono" placeholder="12345678-1-23" {...register('tax_number')} />
        </div>
        <div>
          <label className="label" htmlFor="eu_tax_number">{t('euTaxNumber')}</label>
          <input id="eu_tax_number" className="input font-mono" {...register('eu_tax_number')} />
        </div>
        <div>
          <label className="label" htmlFor="country">{t('country')}</label>
          <input id="country" className="input font-mono" maxLength={2} {...register('country')} />
          {errors.country && <p className="mt-1 text-metadata text-steel-900">{tv('required')}</p>}
        </div>
        <div>
          <label className="label" htmlFor="default_currency">{t('defaultCurrency')}</label>
          <select id="default_currency" className="input" {...register('default_currency')}>
            {currencies.map((c) => (
              <option key={c.key} value={c.key}>
                {c.key}
              </option>
            ))}
          </select>
        </div>
        <div>
          <label className="label" htmlFor="email">{tc('email')}</label>
          <input id="email" type="email" className="input" {...register('email')} />
        </div>
        <div>
          <label className="label" htmlFor="phone">{tc('phone')}</label>
          <input id="phone" type="tel" className="input" {...register('phone')} />
        </div>
        <div>
          <label className="label" htmlFor="website">{t('website')}</label>
          <input id="website" className="input" {...register('website')} />
        </div>
        <div>
          <label className="label" htmlFor="postal_code">{t('postalCode')}</label>
          <input id="postal_code" className="input font-mono" {...register('postal_code')} />
        </div>
        <div>
          <label className="label" htmlFor="city">{t('city')}</label>
          <input id="city" className="input" {...register('city')} />
        </div>
        <div>
          <label className="label" htmlFor="address_line">{t('addressLine')}</label>
          <input id="address_line" className="input" {...register('address_line')} />
        </div>
        <div className="md:col-span-2">
          <label className="label" htmlFor="notes">{t('notes')}</label>
          <textarea id="notes" rows={3} className="input" {...register('notes')} />
        </div>
      </div>
      <div className="card-footer justify-end">
        <button className="btn-primary" type="submit" disabled={isSubmitting}>
          {isSubmitting ? tc('saving') : submitLabel}
        </button>
      </div>
    </form>
  );
}
