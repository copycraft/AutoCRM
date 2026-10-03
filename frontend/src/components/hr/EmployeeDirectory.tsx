'use client';

import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import * as Dialog from '@radix-ui/react-dialog';
import { Plus, Search, UserRound } from 'lucide-react';
import { hrApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { ApiError, errorMessage } from '@/lib/api/errors';
import { canAccessHr, useAuth } from '@/lib/auth/context';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { useRestoreFocus } from '@/hooks/useRestoreFocus';
import { ErrorState } from '@/components/ui/ErrorState';
import { LoadingState } from '@/components/ui/LoadingState';
import { EmptyState } from '@/components/ui/EmptyState';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { ConfirmDialog } from '@/components/ui/ConfirmDialog';
import { EmailValue, PhoneValue } from '@/components/ui/ContactLinks';
import { useToast } from '@/components/ui/Toasts';
import type { Employee } from '@/lib/api/types';

const MAX_PHOTO_BYTES = 8 * 1024 * 1024;

function initials(name: string): string {
  return name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => w[0]!.toUpperCase())
    .join('');
}

export function Avatar({ employee, size }: { employee: Pick<Employee, 'full_name' | 'photo_url'>; size: number }) {
  const style = { width: size, height: size };
  if (employee.photo_url) {
    return (
      // A short-lived signed object-store link, so next/image's allow-list does not apply.
      // eslint-disable-next-line @next/next/no-img-element
      <img
        src={employee.photo_url}
        alt=""
        style={style}
        className="shrink-0 rounded-full border border-steel-200 object-cover"
      />
    );
  }
  return (
    <span
      style={style}
      aria-hidden
      className="flex shrink-0 items-center justify-center rounded-full bg-panel text-steel-500"
    >
      {initials(employee.full_name) || <UserRound className="h-1/2 w-1/2" />}
    </span>
  );
}

/** Gate: the backend refuses everyone else too, this only spares them a request and a 403. */
export function EmployeeDirectory() {
  const { user, isLoading } = useAuth();
  if (isLoading) return <LoadingState />;
  if (!canAccessHr(user)) return <ErrorState error={new ApiError('forbidden', 403, 'forbidden')} />;
  return <Directory />;
}

export function Directory() {
  const t = useTranslations('hr');
  const tc = useTranslations('common');
  const [q, setQ] = useState('');
  const [archived, setArchived] = useState(false);
  const [editing, setEditing] = useState<Employee | 'new' | null>(null);
  const debouncedQ = useDebouncedValue(q);

  const query = useQuery({
    queryKey: qk.employees({ q: debouncedQ, archived }),
    queryFn: () =>
      hrApi.list({ q: debouncedQ.trim() || undefined, include_archived: archived || undefined }),
  });
  const items = query.data?.items ?? [];

  return (
    <div className="mt-6 space-y-4">
      <div className="flex flex-wrap items-center gap-3">
        <div className="relative min-w-64 flex-1 max-w-md">
          <Search
            className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-steel-500"
            aria-hidden
          />
          <input
            type="search"
            className="input pl-9"
            placeholder={t('search')}
            aria-label={t('search')}
            value={q}
            onChange={(e) => setQ(e.target.value)}
          />
        </div>
        <label className="flex items-center gap-2 text-body">
          <input type="checkbox" checked={archived} onChange={(e) => setArchived(e.target.checked)} />
          {t('showArchived')}
        </label>
        <button className="btn-primary ml-auto" onClick={() => setEditing('new')}>
          <Plus className="h-4 w-4" aria-hidden />
          {t('newEmployee')}
        </button>
      </div>

      {query.isLoading ? (
        <LoadingState label={tc('loading')} />
      ) : query.isError ? (
        <ErrorState error={query.error} onRetry={() => void query.refetch()} />
      ) : items.length === 0 ? (
        <EmptyState
          title={debouncedQ.trim() ? t('noMatch') : t('empty')}
          hint={debouncedQ.trim() ? undefined : t('emptyHint')}
        />
      ) : (
        <ul className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
          {items.map((e) => (
            <li key={e.id} className="card">
              <div className="card-content flex gap-4">
                <Avatar employee={e} size={72} />
                <div className="min-w-0 flex-1 space-y-1">
                  <div className="flex items-start justify-between gap-2">
                    <p className="truncate text-section font-semibold">{e.full_name}</p>
                    {e.archived_at && <StatusBadge tone="steel">{t('archived')}</StatusBadge>}
                  </div>
                  <dl className="grid grid-cols-[auto,1fr] gap-x-3 gap-y-0.5 text-body">
                    <dt className="text-steel-500">{t('companyPhone')}</dt>
                    <dd className="truncate">
                      <PhoneValue value={e.company_phone} />
                    </dd>
                    <dt className="text-steel-500">{t('personalPhone')}</dt>
                    <dd className="truncate">
                      <PhoneValue value={e.personal_phone} />
                    </dd>
                    <dt className="text-steel-500">{t('email')}</dt>
                    <dd className="truncate">
                      <EmailValue value={e.email} />
                    </dd>
                  </dl>
                </div>
              </div>
              <div className="card-footer justify-end">
                <button className="btn-ghost btn-sm" onClick={() => setEditing(e)}>
                  {tc('edit')}
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}

      {editing && (
        <EmployeeDialog
          // A fresh form per record: no state leaks from one employee to the next.
          key={editing === 'new' ? 'new' : editing.id}
          employee={editing === 'new' ? null : editing}
          onClose={() => setEditing(null)}
        />
      )}
    </div>
  );
}

function EmployeeDialog({ employee, onClose }: { employee: Employee | null; onClose: () => void }) {
  const t = useTranslations('hr');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const toast = useToast();
  const fileRef = useRef<HTMLInputElement>(null);
  useRestoreFocus(true);

  const [fullName, setFullName] = useState(employee?.full_name ?? '');
  const [email, setEmail] = useState(employee?.email ?? '');
  const [companyPhone, setCompanyPhone] = useState(employee?.company_phone ?? '');
  const [personalPhone, setPersonalPhone] = useState(employee?.personal_phone ?? '');
  const [leaveDays, setLeaveDays] = useState(String(employee?.annual_leave_days ?? 20));
  const [photo, setPhoto] = useState<File | null>(null);
  const [removePhoto, setRemovePhoto] = useState(false);
  const [confirmArchive, setConfirmArchive] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const preview = useMemo(() => (photo ? URL.createObjectURL(photo) : null), [photo]);
  useEffect(() => () => { if (preview) URL.revokeObjectURL(preview); }, [preview]);
  const shown = {
    full_name: fullName,
    photo_url: removePhoto ? null : (preview ?? employee?.photo_url ?? null),
  };

  const done = () => {
    void qc.invalidateQueries({ queryKey: ['employees'] });
    onClose();
  };
  const fail = (e: unknown) => setError(errorMessage(e, ter, ter('unknownError')));

  const save = useMutation({
    mutationFn: async () => {
      // Empty string clears a field on PATCH; on create the backend treats it as absent.
      const body = {
        full_name: fullName,
        email: email.trim() || null,
        company_phone: companyPhone.trim() || null,
        personal_phone: personalPhone.trim() || null,
        annual_leave_days: Number.parseInt(leaveDays, 10),
      };
      const saved = employee ? await hrApi.update(employee.id, body) : await hrApi.create(body);
      if (photo) await hrApi.setPhoto(saved.id, photo);
      else if (removePhoto && employee?.photo_url) await hrApi.removePhoto(saved.id);
    },
    onSuccess: () => {
      toast.success(t('saved'));
      done();
    },
    onError: fail,
  });

  const toggleArchive = useMutation({
    mutationFn: () => (employee!.archived_at ? hrApi.unarchive(employee!.id) : hrApi.archive(employee!.id)),
    onSuccess: done,
    onError: (e) => {
      setConfirmArchive(false);
      fail(e);
    },
  });

  const pick = (file: File | undefined) => {
    if (!file) return;
    if (file.size > MAX_PHOTO_BYTES) {
      setError(t('photoTooBig'));
      return;
    }
    setError(null);
    setPhoto(file);
    setRemovePhoto(false);
  };

  const busy = save.isPending || toggleArchive.isPending;
  const leaveOk = /^\d{1,3}$/.test(leaveDays) && Number.parseInt(leaveDays, 10) <= 366;
  const canSave = fullName.trim() !== '' && leaveOk && !busy;

  return (
    <>
      <Dialog.Root open onOpenChange={(open) => !open && !busy && onClose()}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-50 bg-steel-900/40" />
          <Dialog.Content className="card fixed left-1/2 top-1/2 z-50 max-h-[90vh] w-[92vw] max-w-lg -translate-x-1/2 -translate-y-1/2 overflow-y-auto">
            <form
              onSubmit={(e) => {
                e.preventDefault();
                if (canSave) save.mutate();
              }}
            >
              <div className="card-header">
                <Dialog.Title className="text-section font-semibold">
                  {employee ? t('editEmployee') : t('newEmployee')}
                </Dialog.Title>
                <Dialog.Description className="sr-only">{t('subtitle')}</Dialog.Description>
              </div>
              <div className="card-content space-y-4">
                {error && (
                  <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">
                    {error}
                  </p>
                )}
                <div className="flex items-center gap-4">
                  <Avatar employee={shown} size={88} />
                  <div className="space-y-1">
                    <input
                      ref={fileRef}
                      type="file"
                      accept="image/jpeg,image/png,image/webp"
                      className="sr-only"
                      aria-label={t('photo')}
                      onChange={(e) => pick(e.target.files?.[0])}
                    />
                    <div className="flex gap-2">
                      <button type="button" className="btn-secondary btn-sm" onClick={() => fileRef.current?.click()}>
                        {shown.photo_url ? t('changePhoto') : t('choosePhoto')}
                      </button>
                      {shown.photo_url && (
                        <button
                          type="button"
                          className="btn-ghost btn-sm"
                          onClick={() => {
                            setPhoto(null);
                            setRemovePhoto(true);
                          }}
                        >
                          {t('removePhoto')}
                        </button>
                      )}
                    </div>
                    <p className="text-metadata text-steel-500">{t('photoHint')}</p>
                  </div>
                </div>
                <div>
                  <label className="label" htmlFor="hr-name">
                    {t('fullName')} *
                  </label>
                  <input
                    id="hr-name"
                    className="input"
                    value={fullName}
                    maxLength={200}
                    onChange={(e) => setFullName(e.target.value)}
                    autoFocus
                  />
                </div>
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                  <div>
                    <label className="label" htmlFor="hr-company-phone">
                      {t('companyPhone')}
                    </label>
                    <input
                      id="hr-company-phone"
                      type="tel"
                      className="input"
                      value={companyPhone}
                      maxLength={50}
                      onChange={(e) => setCompanyPhone(e.target.value)}
                    />
                  </div>
                  <div>
                    <label className="label" htmlFor="hr-personal-phone">
                      {t('personalPhone')}
                    </label>
                    <input
                      id="hr-personal-phone"
                      type="tel"
                      className="input"
                      value={personalPhone}
                      maxLength={50}
                      onChange={(e) => setPersonalPhone(e.target.value)}
                    />
                  </div>
                </div>
                <div>
                  <label className="label" htmlFor="hr-leave">
                    {t('annualLeaveDays')}
                  </label>
                  <input
                    id="hr-leave"
                    inputMode="numeric"
                    className="input w-32"
                    value={leaveDays}
                    onChange={(e) => setLeaveDays(e.target.value)}
                    aria-invalid={!leaveOk}
                  />
                  <p className="mt-1 text-metadata text-steel-500">{t('annualLeaveHint')}</p>
                </div>
                <div>
                  <label className="label" htmlFor="hr-email">
                    {t('email')}
                  </label>
                  <input
                    id="hr-email"
                    type="email"
                    className="input"
                    value={email}
                    maxLength={300}
                    onChange={(e) => setEmail(e.target.value)}
                  />
                </div>
              </div>
              <div className="card-footer">
                {employee && (
                  <button
                    type="button"
                    className="btn-ghost mr-auto"
                    disabled={busy}
                    onClick={() => (employee.archived_at ? toggleArchive.mutate() : setConfirmArchive(true))}
                  >
                    {employee.archived_at ? t('unarchive') : t('archive')}
                  </button>
                )}
                <button type="button" className="btn-ghost" onClick={onClose} disabled={busy}>
                  {tc('cancel')}
                </button>
                <button type="submit" className="btn-primary" disabled={!canSave}>
                  {save.isPending ? tc('saving') : tc('save')}
                </button>
              </div>
            </form>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
      <ConfirmDialog
        open={confirmArchive}
        title={t('archiveConfirmTitle')}
        body={t('archiveConfirmBody')}
        onConfirm={() => toggleArchive.mutate()}
        onClose={() => setConfirmArchive(false)}
        busy={toggleArchive.isPending}
      />
    </>
  );
}
