'use client';

import { useState } from 'react';
import { useForm, Controller } from 'react-hook-form';
import { z } from 'zod';
import { zodResolver } from '@hookform/resolvers/zod';
import { useTranslations } from 'next-intl';
import { skipToken, useQuery } from '@tanstack/react-query';
import { errorMessage } from '@/lib/api/errors';
import { partnersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { PartnerPicker, type PartnerOption } from './PartnerPicker';
import { AssigneeField } from './AssigneeField';
import type { Lead, LeadBody } from '@/lib/api/types';

const schema = z.object({
  title: z.string().trim().min(1),
  partner: z.custom<PartnerOption | null>(() => true),
  contact_id: z.string(),
  contact_name: z.string().trim().optional(),
  contact_email: z.string().trim().optional(),
  contact_phone: z.string().trim().optional(),
  source: z.string().trim().optional(),
  description: z.string().trim().optional(),
  assigned_to: z.custom<number | null | 'me'>(() => true),
});

export type LeadFormValues = z.infer<typeof schema>;

function assignee(v: LeadFormValues['assigned_to'], meId: number | undefined): number | null {
  return v === 'me' ? (meId ?? null) : v;
}

export function leadCreateBody(v: LeadFormValues, meId: number | undefined): LeadBody {
  const clean = (s: string | undefined) => (s?.trim() ? s.trim() : undefined);
  const assigned = assignee(v.assigned_to, meId);
  return {
    title: v.title.trim(),
    partner_id: v.partner?.id ?? null,
    contact_id: v.contact_id ? Number(v.contact_id) : null,
    contact_name: clean(v.contact_name),
    contact_email: clean(v.contact_email),
    contact_phone: clean(v.contact_phone),
    source: clean(v.source),
    description: clean(v.description),
    assigned_to: assigned,
  };
}

/** PATCH diff: omit unchanged, null clears (backend blank→null). */
export function leadPatchBody(original: Lead, v: LeadFormValues, meId: number | undefined): LeadBody {
  const body: LeadBody = {};
  if (v.title.trim() !== original.title) body.title = v.title.trim();
  const origPartner = original.partner_id ?? null;
  if ((v.partner?.id ?? null) !== origPartner) body.partner_id = v.partner?.id ?? null;
  const newContact = v.contact_id ? Number(v.contact_id) : null;
  if (newContact !== (original.contact_id ?? null)) body.contact_id = newContact;
  for (const f of ['contact_name', 'contact_email', 'contact_phone', 'source', 'description'] as const) {
    const nv = v[f]?.trim() ?? '';
    if (nv !== (original[f] ?? '')) body[f] = nv ? nv : null;
  }
  const assigned = assignee(v.assigned_to, meId);
  if (assigned !== (original.assigned_to ?? null)) body.assigned_to = assigned;
  return body;
}

export function LeadForm({
  initial,
  initialPartner,
  onSubmit,
  submitLabel,
}: {
  initial?: Lead;
  initialPartner?: PartnerOption | null;
  onSubmit: (v: LeadFormValues) => Promise<void>;
  submitLabel: string;
}) {
  const t = useTranslations('leads');
  const tc = useTranslations('common');
  const tv = useTranslations('validation');
  const ter = useTranslations('errors');
  const [serverError, setServerError] = useState<string | null>(null);

  const { register, handleSubmit, control, watch, formState } = useForm<LeadFormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      title: initial?.title ?? '',
      partner: initialPartner ?? null,
      contact_id: initial?.contact_id ? String(initial.contact_id) : '',
      contact_name: initial?.contact_name ?? '',
      contact_email: initial?.contact_email ?? '',
      contact_phone: initial?.contact_phone ?? '',
      source: initial?.source ?? '',
      description: initial?.description ?? '',
      assigned_to: initial?.assigned_to ?? null,
    },
  });

  const partner = watch('partner');
  const partnerId = partner?.id;
  const contactsQuery = useQuery({
    queryKey: qk.partner(partnerId ?? 0),
    queryFn: partnerId === undefined ? skipToken : () => partnersApi.get(partnerId),
  });

  return (
    <form
      className="card"
      noValidate
      onSubmit={handleSubmit(async (v) => {
        setServerError(null);
        try {
          await onSubmit(v);
        } catch (e) {
          setServerError(errorMessage(e, ter, ter('unknownError')));
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
          <label className="label" htmlFor="lf-title">{t('title')} *</label>
          <input id="lf-title" className="input" {...register('title')} />
          {formState.errors.title && <p className="mt-1 text-xs text-steel-900">{tv('required')}</p>}
        </div>
        <Controller
          control={control}
          name="partner"
          render={({ field }) => (
            <PartnerPicker value={field.value} onChange={field.onChange} label={t('partner')} />
          )}
        />
        <div>
          <label className="label" htmlFor="lf-contact">{t('contact')}</label>
          <select id="lf-contact" className="input" {...register('contact_id')} disabled={!partner || contactsQuery.isLoading}>
            <option value="">—</option>
            {contactsQuery.data?.contacts
              .filter((c) => !c.archived_at)
              .map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
          </select>
          {contactsQuery.isError && (
            <p className="mt-1 text-xs text-steel-900" role="alert">
              {errorMessage(contactsQuery.error, ter, ter('unknownError'))}
            </p>
          )}
        </div>
        <div>
          <label className="label" htmlFor="lf-cname">{t('contactName')}</label>
          <input id="lf-cname" className="input" {...register('contact_name')} />
        </div>
        <div>
          <label className="label" htmlFor="lf-cemail">{t('contactEmail')}</label>
          <input id="lf-cemail" type="email" className="input" {...register('contact_email')} />
        </div>
        <div>
          <label className="label" htmlFor="lf-cphone">{t('contactPhone')}</label>
          <input id="lf-cphone" type="tel" className="input" {...register('contact_phone')} />
        </div>
        <div>
          <label className="label" htmlFor="lf-source">{t('source')}</label>
          <input id="lf-source" className="input" placeholder="web / telefon / …" {...register('source')} />
        </div>
        <div className="md:col-span-2">
          <label className="label" htmlFor="lf-desc">{t('description')}</label>
          <textarea id="lf-desc" rows={3} className="input" {...register('description')} />
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
          {formState.isSubmitting ? tc('saving') : submitLabel}
        </button>
      </div>
    </form>
  );
}
