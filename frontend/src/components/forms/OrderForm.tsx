'use client';

import { useState } from 'react';
import { useForm, Controller } from 'react-hook-form';
import { z } from 'zod';
import { zodResolver } from '@hookform/resolvers/zod';
import { useTranslations } from 'next-intl';
import { skipToken, useQuery } from '@tanstack/react-query';
import { constraintName, errorMessage } from '@/lib/api/errors';
import { configApi, partnersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { PartnerPicker, type PartnerOption } from './PartnerPicker';
import { AssigneeField } from './AssigneeField';
import { DateQuickPicks } from './DateQuickPicks';
import { lastAssignee, lastUsed } from '@/hooks/useLastUsed';
import { useLookups } from '@/hooks/useLookups';
import { useDirtyGuard } from '@/hooks/useDirtyGuard';
import { useFormDraft } from '@/hooks/useFormDraft';
import { VinDecodeButton } from '@/components/orders/VehicleExtras';
import { QuestionnaireFields } from '@/components/spec/QuestionnaireFields';
import { useQuestionnaire } from '@/components/spec/QuestionnaireView';
import { ReturningVehicleWarning } from '@/components/orders/ReturningVehicleWarning';
import type { Answers, Order, OrderBody, PatchOrder } from '@/lib/api/types';

/**
 * A react-hook-form error message as Hungarian text.
 *
 * Three kinds arrive here: a `range:min:max` token (`answerProblem` in lib/questionnaire), a bare
 * catalogue key, and a message the server wrote (already Hungarian, or at least already
 * a sentence). Anything unrecognised is shown as-is rather than swallowed — a message
 * the user can read beats a blank space under a red box.
 */
export function fieldErrorText(
  message: string | undefined,
  tv: (key: string, values?: Record<string, string | number>) => string,
): string {
  if (!message) return '';
  const range = /^range:(-?[\d.]+):(-?[\d.]+)$/.exec(message);
  if (range) return tv('range', { min: range[1] ?? '', max: range[2] ?? '' });
  let translated = message;
  try {
    translated = tv(message);
  } catch {
    // Not a catalogue key — a server sentence, or a zod default.
  }
  return translated === message ? message : translated;
}

/**
 * Database constraint → the input that carries the offending value. The last line of
 * defence: mapping the name back to a field turns "the data violates the database's rules"
 * into a red box around the thing to change. (The questionnaire's answers are checked by
 * the server against their questions and come back as a message naming the question.)
 */
const CONSTRAINT_FIELD: Record<string, keyof OrderFormValues> = {
  orders_currency_check: 'currency',
};

const schema = z.object({
  title: z.string().trim().min(1),
  // Partner is required, but enforced in the submit handler (not via
  // .refine) so the field type stays a plain nullable the form can handle.
  partner: z.custom<PartnerOption | null>(() => true),
  contact_id: z.string(),
  project_type_id: z.string(),
  // The server owns the currency list (`GET /config/lookups`) and validates;
  // the form only requires something chosen.
  currency: z.string().min(1),
  valuation_date: z.string(),
  vehicle_make: z.string().trim().optional(),
  vehicle_model: z.string().trim().optional(),
  vehicle_plate: z.string().trim().optional(),
  vehicle_vin: z.string().trim().optional(),
  description: z.string().trim().optional(),
  due_date: z.string(),
  assigned_to: z.custom<number | null | 'me'>(() => true),
  // The build questionnaire's answers (key → answer). Which questions show depends on the
  // project type and on the answers themselves; the server checks every value.
  spec_answers: z.custom<Answers>(() => true),
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
    currency: v.currency as OrderBody['currency'],
    valuation_date: date(v.valuation_date),
    vehicle_make: clean(v.vehicle_make),
    vehicle_model: clean(v.vehicle_model),
    vehicle_plate: clean(v.vehicle_plate),
    vehicle_vin: clean(v.vehicle_vin),
    description: clean(v.description),
    due_date: date(v.due_date),
    assigned_to: assignee(v.assigned_to, meId),
    spec_answers: v.spec_answers,
    items: [],
  };
}

/**
 * PATCH diff with backend semantics. Currency is sent only when it changed
 * and the order has no items (the backend answers 422 currency_locked
 * otherwise — the control is disabled in that case, so this is a backstop).
 * Changing partner without explicitly picking a contact clears the contact,
 * mirroring the backend rule.
 */
export function orderPatchBody(
  original: Order,
  v: OrderFormValues,
  meId: number | undefined,
  contactDirty: boolean,
  currencyLocked: boolean,
): PatchOrder {
  const body: PatchOrder = {};
  if (v.title.trim() !== original.title) body.title = v.title.trim();
  if (!currencyLocked && v.currency !== original.currency) {
    body.currency = v.currency as OrderBody['currency'];
  }
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
  initialAnswers,
  currencyLocked,
  draftKey,
  onSubmit,
  submitLabel,
}: {
  initial?: Order;
  initialPartner?: PartnerOption | null;
  /** The order's questionnaire answers, so editing starts from what was recorded. */
  initialAnswers?: Answers;
  /** True while the order has items — currency select disabled. */
  currencyLocked?: boolean;
  /** Draft autosave slot for the create page; ignored when editing. */
  draftKey?: string;
  onSubmit: (v: OrderFormValues, contactDirty: boolean) => Promise<void>;
  submitLabel: string;
}) {
  const t = useTranslations('orders');
  const tc = useTranslations('common');
  const tq = useTranslations('qol');
  const tv = useTranslations('validation');
  const ter = useTranslations('errors');
  const ts = useTranslations('spec');
  const [serverError, setServerError] = useState<string | null>(null);
  const [partnerError, setPartnerError] = useState(false);
  const { data: lookups } = useLookups();
  const currencies = lookups?.currencies ?? [];

  const emptyOrder: OrderFormValues = {
    title: '',
    partner: null,
    contact_id: '',
    project_type_id: '',
    currency: 'HUF',
    valuation_date: todayLocal(),
    vehicle_make: '',
    vehicle_model: '',
    vehicle_plate: '',
    vehicle_vin: '',
    description: '',
    due_date: '',
    assigned_to: null,
    spec_answers: {},
  };

  const { register, handleSubmit, control, watch, reset, setValue, setError, formState } = useForm<OrderFormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      title: initial?.title ?? '',
      partner: initialPartner ?? null,
      contact_id: initial?.contact_id ? String(initial.contact_id) : '',
      project_type_id:
        initial?.project_type_id ? String(initial.project_type_id) : (lastUsed('ptype') ?? ''),
      currency: initial?.currency ?? 'HUF',
      valuation_date: initial?.valuation_date ?? todayLocal(),
      vehicle_make: initial?.vehicle_make ?? '',
      vehicle_model: initial?.vehicle_model ?? '',
      vehicle_plate: initial?.vehicle_plate ?? '',
      vehicle_vin: initial?.vehicle_vin ?? '',
      description: initial?.description ?? '',
      due_date: initial?.due_date ?? '',
      assigned_to: initial?.assigned_to ?? lastAssignee(),
      spec_answers: initialAnswers ?? {},
    },
  });

  const partner = watch('partner');
  const partnerId = partner?.id;
  const draft = useFormDraft({
    key: draftKey && !initial ? draftKey : null,
    watch,
    reset,
    empty: emptyOrder,
  });
  // The answers are a field of this same form, so its isDirty covers them too.
  useDirtyGuard(formState.isDirty && !formState.isSubmitSuccessful, tq('unsavedChanges'));
  const contactsQuery = useQuery({
    queryKey: qk.partner(partnerId ?? 0),
    queryFn: partnerId === undefined ? skipToken : () => partnersApi.get(partnerId),
  });
  const projectTypes = useQuery({
    queryKey: qk.projectTypes,
    queryFn: () => configApi.projectTypes(),
  });
  // The questionnaire below the vehicle fields follows the project type and its own
  // answers: "Fűteni kell?" igen opens the heating questions, and so on.
  const questionnaire = useQuestionnaire();
  const projectTypeId = watch('project_type_id') ? Number(watch('project_type_id')) : null;

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
          draft.clear();
        } catch (e) {
          // A rejected write names the constraint it tripped. If that names a field on
          // this form, put the error there and take the user to it; the banner is for
          // everything that genuinely has no field to blame.
          const field = CONSTRAINT_FIELD[constraintName(e) ?? ''];
          if (field) {
            setError(field, { message: 'invalidValue' }, { shouldFocus: true });
          } else {
            setServerError(errorMessage(e, ter, ter('unknownError')));
          }
        }
      })}
    >
      {serverError && (
        <div className="card-content pb-0">
          <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">{serverError}</p>
        </div>
      )}
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
      <div className="card-content grid grid-cols-1 gap-4 md:grid-cols-2">
        <div className="md:col-span-2">
          <label className="label" htmlFor="of-title">{t('fieldTitle')} *</label>
          <input id="of-title" className="input" {...register('title')} />
          {formState.errors.title && <p className="mt-1 text-metadata text-steel-900">{tv('required')}</p>}
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
                role="customer"
              />
              {partnerError && <p className="mt-1 text-metadata text-steel-900">{tv('required')}</p>}
            </div>
          )}
        />
        <div>
          <label className="label" htmlFor="of-currency">{t('currencyLabel')} *</label>
          <select id="of-currency" className="input" {...register('currency')} disabled={currencyLocked}>
            {currencies.map((c) => (
              <option key={c.key} value={c.key}>
                {c.key}
              </option>
            ))}
          </select>
          {currencyLocked && <p className="mt-1 text-metadata text-steel-500">{t('currencyLocked')}</p>}
        </div>
        <div>
          <label className="label" htmlFor="of-contact">{tc('contact')}</label>
          <select id="of-contact" className="input" {...register('contact_id')} disabled={!partner || contactsQuery.isLoading}>
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
            <p className="mt-1 text-metadata text-steel-900" role="alert">
              {errorMessage(contactsQuery.error, ter, ter('unknownError'))}
            </p>
          )}
          {initial && <p className="mt-1 text-metadata text-steel-500">{t('contactClearedNote')}</p>}
        </div>
        <div>
          <label className="label" htmlFor="of-pt">{t('projectType')}</label>
          <select id="of-pt" className="input" {...register('project_type_id')} disabled={projectTypes.isLoading}>
            <option value="">—</option>
            {(projectTypes.data?.items ?? [])
              .filter((p) => p.is_active)
              .map((p) => (
                <option key={p.id} value={p.id}>
                  {p.label_hu}
                </option>
              ))}
          </select>
          {projectTypes.isError && (
            <p className="mt-1 text-metadata text-steel-900" role="alert">
              {errorMessage(projectTypes.error, ter, ter('unknownError'))}
            </p>
          )}
        </div>
        <div>
          <label className="label" htmlFor="of-val">{t('valuationDate')}</label>
          <input id="of-val" type="date" className="input font-mono" {...register('valuation_date')} />
        </div>
        <div>
          <label className="label" htmlFor="of-due">{t('dueDate')}</label>
          <input id="of-due" type="date" className="input font-mono" {...register('due_date')} />
          <div className="mt-1">
            <DateQuickPicks onPick={(iso) => setValue('due_date', iso, { shouldDirty: true, shouldTouch: true })} />
          </div>
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
          <VinDecodeButton
            vin={watch('vehicle_vin') ?? ''}
            onFill={({ make, model }) => {
              if (make && !watch('vehicle_make')) setValue('vehicle_make', make, { shouldDirty: true });
              if (model && !watch('vehicle_model')) setValue('vehicle_model', model, { shouldDirty: true });
            }}
          />
        </div>
        <ReturningVehicleWarning
          plate={watch('vehicle_plate') ?? ''}
          vin={watch('vehicle_vin') ?? ''}
          active={initial == null}
        />
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

        <div className="md:col-span-2 border-t border-steel-200 pt-4">
          <h3 className="text-section font-semibold">{ts('title')}</h3>
          <p className="mt-1 text-metadata text-steel-500">{ts('explanation')}</p>
          <div className="mt-4">
            <Controller
              control={control}
              name="spec_answers"
              render={({ field }) => (
                <QuestionnaireFields
                  questionnaire={questionnaire.data}
                  projectTypeId={projectTypeId}
                  answers={field.value ?? {}}
                  onChange={field.onChange}
                  idPrefix="of"
                />
              )}
            />
          </div>
        </div>
      </div>
      <div className="card-footer justify-end">
        <button className="btn-primary" type="submit" disabled={formState.isSubmitting}>
          {formState.isSubmitting ? tc('saving') : submitLabel}
        </button>
      </div>
    </form>
  );
}
