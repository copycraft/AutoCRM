'use client';

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useForm } from 'react-hook-form';
import { z } from 'zod';
import { zodResolver } from '@hookform/resolvers/zod';
import { partnersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { ConfirmDialog } from '@/components/ui/ConfirmDialog';
import type { Contact, ContactBody } from '@/lib/api/types';

const schema = z.object({
  name: z.string().trim().min(1),
  email: z.string().trim().optional(),
  phone: z.string().trim().optional(),
  position: z.string().trim().optional(),
  notes: z.string().trim().optional(),
});
type Values = z.infer<typeof schema>;

function ContactForm({
  initial,
  onSubmit,
  submitLabel,
  onCancel,
}: {
  initial?: Contact;
  onSubmit: (v: Values) => Promise<void>;
  submitLabel: string;
  onCancel: () => void;
}) {
  const tv = useTranslations('validation');
  const tc = useTranslations('common');
  const [serverError, setServerError] = useState<string | null>(null);
  const { register, handleSubmit, formState } = useForm<Values>({
    resolver: zodResolver(schema),
    defaultValues: {
      name: initial?.name ?? '',
      email: initial?.email ?? '',
      phone: initial?.phone ?? '',
      position: initial?.position ?? '',
      notes: initial?.notes ?? '',
    },
  });
  return (
    <form
      className="rounded-lg border border-steel-200 bg-panel p-4 space-y-3"
      noValidate
      onSubmit={handleSubmit(async (v) => {
        setServerError(null);
        try {
          await onSubmit(v);
        } catch (e) {
          setServerError(errorMessage(e, 'Ismeretlen hiba.'));
        }
      })}
    >
      <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
        <div>
          <label className="label" htmlFor="c-name">Név *</label>
          <input id="c-name" className="input" {...register('name')} />
          {formState.errors.name && <p className="mt-1 text-xs text-signal">{tv('required')}</p>}
        </div>
        <div>
          <label className="label" htmlFor="c-position">Beosztás</label>
          <input id="c-position" className="input" {...register('position')} />
        </div>
        <div>
          <label className="label" htmlFor="c-email">E-mail</label>
          <input id="c-email" type="email" className="input" {...register('email')} />
        </div>
        <div>
          <label className="label" htmlFor="c-phone">Telefon</label>
          <input id="c-phone" type="tel" className="input" {...register('phone')} />
        </div>
        <div className="md:col-span-2">
          <label className="label" htmlFor="c-notes">Megjegyzések</label>
          <textarea id="c-notes" rows={2} className="input" {...register('notes')} />
        </div>
      </div>
      {serverError && (
        <p className="rounded-lg bg-signal/10 px-3 py-2 text-sm text-signal" role="alert">{serverError}</p>
      )}
      <div className="flex justify-end gap-2">
        <button type="button" className="btn-ghost btn-sm" onClick={onCancel}>{tc('cancel')}</button>
        <button type="submit" className="btn-primary btn-sm" disabled={formState.isSubmitting}>
          {formState.isSubmitting ? 'Mentés…' : submitLabel}
        </button>
      </div>
    </form>
  );
}

const clean = (s: string | undefined) => (s?.trim() ? s.trim() : null);

/** PATCH diff: omit unchanged, null clears. */
function contactPatch(original: Contact, v: Values): ContactBody {
  const body: ContactBody = {};
  if (v.name.trim() !== original.name) body.name = v.name.trim();
  for (const f of ['email', 'phone', 'position', 'notes'] as const) {
    const nv = v[f]?.trim() ?? '';
    if (nv !== (original[f] ?? '')) body[f] = nv ? nv : null;
  }
  return body;
}

export function ContactSection({ partnerId, contacts }: { partnerId: number; contacts: Contact[] }) {
  const t = useTranslations('partners');
  const tc = useTranslations('common');
  const qc = useQueryClient();
  const [adding, setAdding] = useState(false);
  const [editing, setEditing] = useState<Contact | null>(null);
  const [archiving, setArchiving] = useState<Contact | null>(null);

  const invalidate = () => {
    void qc.invalidateQueries({ queryKey: qk.partner(partnerId) });
    void qc.invalidateQueries({ queryKey: ['partners'] });
  };

  const create = useMutation({
    mutationFn: (v: Values) =>
      partnersApi.createContact(partnerId, {
        name: v.name.trim(),
        email: clean(v.email),
        phone: clean(v.phone),
        position: clean(v.position),
        notes: clean(v.notes),
      }),
    onSuccess: () => {
      setAdding(false);
      invalidate();
    },
  });

  const patch = useMutation({
    mutationFn: ({ original, v }: { original: Contact; v: Values }) =>
      partnersApi.patchContact(original.id, contactPatch(original, v)),
    onSuccess: () => {
      setEditing(null);
      invalidate();
    },
  });

  const archive = useMutation({
    mutationFn: (id: number) => partnersApi.archiveContact(id),
    onSuccess: () => {
      setArchiving(null);
      invalidate();
    },
  });

  return (
    <section className="card">
      <div className="card-header flex items-center justify-between">
        <h2 className="text-section font-semibold">{t('contacts')} ({contacts.length})</h2>
        {!adding && (
          <button className="btn-secondary btn-sm" onClick={() => setAdding(true)}>
            {t('addContact')}
          </button>
        )}
      </div>
      <div className="card-content space-y-3">
        {adding && (
          <ContactForm
            submitLabel={tc('create')}
            onCancel={() => setAdding(false)}
            onSubmit={(v) => create.mutateAsync(v).then(() => undefined)}
          />
        )}
        {contacts.length === 0 && !adding && (
          <p className="text-sm text-steel-500">{t('noContacts')}</p>
        )}
        {contacts.map((c) => (
          <div key={c.id} className="rounded-lg border border-steel-200 p-4">
            {editing?.id === c.id ? (
              <ContactForm
                initial={c}
                submitLabel={tc('save')}
                onCancel={() => setEditing(null)}
                onSubmit={(v) => patch.mutateAsync({ original: c, v }).then(() => undefined)}
              />
            ) : (
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div>
                  <p className="font-medium">
                    {c.name}
                    {c.archived_at && (
                      <span className="ml-2"><StatusBadge tone="steel">Archivált</StatusBadge></span>
                    )}
                  </p>
                  <p className="text-sm text-steel-500">
                    {[c.position, c.email, c.phone].filter(Boolean).join(' · ') || '—'}
                  </p>
                  {c.notes && <p className="mt-1 text-sm">{c.notes}</p>}
                </div>
                {!c.archived_at && (
                  <div className="flex gap-2">
                    <button className="btn-ghost btn-sm" onClick={() => setEditing(c)}>
                      {tc('edit')}
                    </button>
                    <button className="btn-ghost btn-sm" onClick={() => setArchiving(c)}>
                      {t('archive')}
                    </button>
                  </div>
                )}
              </div>
            )}
          </div>
        ))}
      </div>
      <ConfirmDialog
        open={archiving !== null}
        title={`${t('archive')}: ${archiving?.name ?? ''}`}
        body={t('archiveContactBody')}
        confirmLabel={t('archive')}
        onClose={() => setArchiving(null)}
        busy={archive.isPending}
        onConfirm={() => archiving && archive.mutate(archiving.id)}
      />
    </section>
  );
}
