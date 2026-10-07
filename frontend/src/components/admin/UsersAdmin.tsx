'use client';

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import * as Dialog from '@radix-ui/react-dialog';
import { Plus } from 'lucide-react';
import { accountApi, usersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { ApiError, errorMessage } from '@/lib/api/errors';
import { canAdmin, useAuth } from '@/lib/auth/context';
import { useRestoreFocus } from '@/hooks/useRestoreFocus';
import { ErrorState } from '@/components/ui/ErrorState';
import { LoadingState } from '@/components/ui/LoadingState';
import { EmptyState } from '@/components/ui/EmptyState';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { useToast } from '@/components/ui/Toasts';
import type { Capability, Role, User } from '@/lib/api/types';

const ROLES: Role[] = ['admin', 'office', 'designer', 'viewer'];

/** What an admin can grant one user on top of the role, mirroring Capability::grantable. */
const GRANTABLE: Capability[] = [
  'edit_partners',
  'edit_leads',
  'edit_orders',
  'change_stages',
  'manage_blockers',
  'upload_media',
  'delete_media',
  'view_original_images',
  'comment',
  'send_email',
  'issue_invoices',
  'annul_invoices',
  'manage_configuration',
  'manage_settings',
  'operate_system',
];

/** The role's defaults, mirroring Role::can in domain/role.rs. */
const ROLE_DEFAULTS: Record<Role, Capability[]> = {
  admin: GRANTABLE,
  office: [
    'edit_partners',
    'edit_leads',
    'edit_orders',
    'delete_media',
    'view_original_images',
    'send_email',
    'issue_invoices',
    'change_stages',
    'manage_blockers',
    'upload_media',
    'comment',
  ],
  designer: ['change_stages', 'manage_blockers', 'upload_media', 'comment'],
  viewer: [],
};

/** Gate: the backend refuses everyone else too, this only spares them a request and a 403. */
export function UsersAdmin() {
  const { user, isLoading } = useAuth();
  if (isLoading) return <LoadingState />;
  if (!canAdmin(user)) return <ErrorState error={new ApiError('forbidden', 403, 'forbidden')} />;
  return <UserTable meId={user!.id} />;
}

function UserTable({ meId }: { meId: number }) {
  const t = useTranslations('users');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const toast = useToast();
  const [creating, setCreating] = useState(false);
  const [resetting, setResetting] = useState<User | null>(null);
  const [granting, setGranting] = useState<User | null>(null);

  const query = useQuery({ queryKey: qk.users, queryFn: () => usersApi.list() });

  // One PATCH per change; the row is refreshed from the server's answer, so what the admin
  // sees is what was stored (including the "last admin" refusal).
  const patch = useMutation({
    mutationFn: ({ id, body }: { id: number; body: Parameters<typeof usersApi.update>[1] }) =>
      usersApi.update(id, body),
    onSuccess: () => {
      toast.success(t('saved'));
      void qc.invalidateQueries({ queryKey: qk.users });
      // Granting or taking away your own access changes the menu.
      void qc.invalidateQueries({ queryKey: qk.me });
    },
    onError: (e) => toast.error(errorMessage(e, ter, ter('unknownError'))),
  });

  const revoke = useMutation({
    mutationFn: (id: number) => usersApi.revokeSessions(id),
    onSuccess: () => toast.success(t('sessionsRevoked')),
    onError: (e) => toast.error(errorMessage(e, ter, ter('unknownError'))),
  });

  const ta = useTranslations('account');
  const resetTwoFactor = useMutation({
    mutationFn: (id: number) => accountApi.resetUserTwoFactor(id),
    onSuccess: () => {
      toast.success(t('saved'));
      void qc.invalidateQueries({ queryKey: qk.users });
    },
    onError: (e) => toast.error(errorMessage(e, ter, ter('unknownError'))),
  });

  const users = query.data?.items ?? [];

  return (
    <div className="mt-6 space-y-4">
      <div className="flex items-center justify-between gap-3">
        <p className="max-w-2xl text-metadata text-steel-500">{t('hrAccessHint')}</p>
        <button className="btn-primary shrink-0" onClick={() => setCreating(true)}>
          <Plus className="h-4 w-4" aria-hidden />
          {t('newUser')}
        </button>
      </div>

      {query.isLoading ? (
        <LoadingState label={tc('loading')} />
      ) : query.isError ? (
        <ErrorState error={query.error} onRetry={() => void query.refetch()} />
      ) : users.length === 0 ? (
        <EmptyState title={t('empty')} />
      ) : (
        <div className="card overflow-x-auto">
          <table className="w-full text-body">
            <thead>
              <tr className="border-b border-steel-200 text-left text-metadata text-steel-500">
                <th className="px-4 py-2 font-medium">{tc('name')}</th>
                <th className="px-4 py-2 font-medium">{t('role')}</th>
                <th className="px-4 py-2 font-medium">{t('active')}</th>
                <th className="px-4 py-2 font-medium">{t('hrAccess')}</th>
                <th className="px-4 py-2 font-medium">
                  <span className="sr-only">{tc('actions')}</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {users.map((u) => {
                const self = u.id === meId;
                return (
                  <tr key={u.id} className="border-b border-steel-200 last:border-0">
                    <td className="px-4 py-2">
                      <div className="flex items-center gap-2">
                        <span className="font-medium">{u.display_name}</span>
                        {self && <StatusBadge tone="cold">{t('you')}</StatusBadge>}
                        {!u.is_active && <StatusBadge tone="steel">{t('inactive')}</StatusBadge>}
                        {u.two_factor && <StatusBadge tone="cold">{ta('twoFactorBadge')}</StatusBadge>}
                      </div>
                      <div className="text-metadata text-steel-500">{u.email}</div>
                    </td>
                    <td className="px-4 py-2">
                      <select
                        className="input w-auto"
                        aria-label={`${t('role')}: ${u.display_name}`}
                        value={u.role}
                        // Not your own: demoting yourself is the quickest way to lock out.
                        disabled={self || patch.isPending}
                        onChange={(e) => patch.mutate({ id: u.id, body: { role: e.target.value as Role } })}
                      >
                        {ROLES.map((r) => (
                          <option key={r} value={r}>
                            {t(`roles.${r}`)}
                          </option>
                        ))}
                      </select>
                    </td>
                    <td className="px-4 py-2">
                      <input
                        type="checkbox"
                        aria-label={`${t('active')}: ${u.display_name}`}
                        checked={u.is_active}
                        disabled={self || patch.isPending}
                        onChange={(e) => patch.mutate({ id: u.id, body: { is_active: e.target.checked } })}
                      />
                    </td>
                    <td className="px-4 py-2">
                      {u.role === 'admin' ? (
                        <span className="text-steel-500">{t('hrAccessAlways')}</span>
                      ) : (
                        <input
                          type="checkbox"
                          aria-label={`${t('hrAccess')}: ${u.display_name}`}
                          checked={u.hr_access}
                          disabled={patch.isPending}
                          onChange={(e) => patch.mutate({ id: u.id, body: { hr_access: e.target.checked } })}
                        />
                      )}
                    </td>
                    <td className="px-4 py-2 text-right">
                      {u.role !== 'admin' && (
                        <button className="btn-ghost btn-sm" onClick={() => setGranting(u)}>
                          {t('permissions')}
                          {u.permissions.length > 0 && ' ' + t('permissionsExtra', { count: u.permissions.length })}
                        </button>
                      )}
                      <button className="btn-ghost btn-sm" onClick={() => setResetting(u)}>
                        {t('resetPassword')}
                      </button>
                      <button
                        className="btn-ghost btn-sm"
                        disabled={revoke.isPending}
                        onClick={() => revoke.mutate(u.id)}
                      >
                        {t('revokeSessions')}
                      </button>
                      {u.two_factor && !self && (
                        <button
                          className="btn-ghost btn-sm"
                          disabled={resetTwoFactor.isPending}
                          onClick={() => {
                            if (window.confirm(ta('resetTwoFactorConfirm', { name: u.display_name }))) resetTwoFactor.mutate(u.id);
                          }}
                        >
                          {ta('resetTwoFactor')}
                        </button>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      {creating && <CreateUserDialog onClose={() => setCreating(false)} />}
      {resetting && <ResetPasswordDialog user={resetting} onClose={() => setResetting(null)} />}
      {granting && <PermissionsDialog user={granting} onClose={() => setGranting(null)} />}
    </div>
  );
}

function PasswordField({ id, label, value, onChange }: { id: string; label: string; value: string; onChange: (v: string) => void }) {
  return (
    <div>
      <label className="label" htmlFor={id}>
        {label} *
      </label>
      <input
        id={id}
        type="text"
        autoComplete="off"
        className="input font-mono"
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}

function FormDialog({
  title,
  description,
  error,
  busy,
  canSubmit,
  submitLabel,
  onSubmit,
  onClose,
  children,
}: {
  title: string;
  description?: string;
  error: string | null;
  busy: boolean;
  canSubmit: boolean;
  submitLabel: string;
  onSubmit: () => void;
  onClose: () => void;
  children: React.ReactNode;
}) {
  const tc = useTranslations('common');
  useRestoreFocus(true);
  return (
    <Dialog.Root open onOpenChange={(open) => !open && !busy && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-steel-900/40" />
        <Dialog.Content className="card fixed left-1/2 top-1/2 z-50 max-h-[90vh] w-[92vw] max-w-md -translate-x-1/2 -translate-y-1/2 overflow-y-auto">
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (canSubmit && !busy) onSubmit();
            }}
          >
            <div className="card-header">
              <Dialog.Title className="text-section font-semibold">{title}</Dialog.Title>
              {description && (
                <Dialog.Description className="mt-1 text-metadata text-steel-500">{description}</Dialog.Description>
              )}
            </div>
            <div className="card-content space-y-4">
              {error && (
                <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">
                  {error}
                </p>
              )}
              {children}
            </div>
            <div className="card-footer justify-end">
              <button type="button" className="btn-ghost" onClick={onClose} disabled={busy}>
                {tc('cancel')}
              </button>
              <button type="submit" className="btn-primary" disabled={!canSubmit || busy}>
                {busy ? tc('processing') : submitLabel}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

/** Per-user grants on top of the role: the role's own capabilities show as fixed. */
function PermissionsDialog({ user, onClose }: { user: User; onClose: () => void }) {
  const t = useTranslations('users');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const toast = useToast();
  const [granted, setGranted] = useState<Set<string>>(new Set(user.permissions));
  const fromRole = new Set<Capability>(ROLE_DEFAULTS[user.role]);

  const save = useMutation({
    mutationFn: () =>
      usersApi.update(user.id, {
        // Only what the role does not already give; the role's set is implied.
        permissions: GRANTABLE.filter((c) => granted.has(c) && !fromRole.has(c)),
      }),
    onSuccess: () => {
      toast.success(t('saved'));
      void qc.invalidateQueries({ queryKey: qk.users });
      void qc.invalidateQueries({ queryKey: qk.me });
      onClose();
    },
  });

  return (
    <FormDialog
      title={t('permissionsTitle', { name: user.display_name })}
      description={t('permissionsHint')}
      error={save.error ? errorMessage(save.error, ter, ter('unknownError')) : null}
      busy={save.isPending}
      canSubmit
      submitLabel={tc('save')}
      onSubmit={() => save.mutate()}
      onClose={onClose}
    >
      <ul className="space-y-2">
        {GRANTABLE.map((c) => {
          const inherited = fromRole.has(c);
          return (
            <li key={c}>
              <label className="flex items-center gap-2 text-body">
                <input
                  type="checkbox"
                  checked={inherited || granted.has(c)}
                  disabled={inherited || save.isPending}
                  onChange={(e) => {
                    const next = new Set(granted);
                    if (e.target.checked) next.add(c);
                    else next.delete(c);
                    setGranted(next);
                  }}
                />
                <span>{t(`capabilities.${c}`)}</span>
                {inherited && <span className="text-metadata text-steel-500">({t('permissionFromRole')})</span>}
              </label>
            </li>
          );
        })}
      </ul>
    </FormDialog>
  );
}

function CreateUserDialog({ onClose }: { onClose: () => void }) {
  const t = useTranslations('users');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const toast = useToast();
  const [email, setEmail] = useState('');
  const [displayName, setDisplayName] = useState('');
  const [role, setRole] = useState<Role>('viewer');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);

  const create = useMutation({
    mutationFn: () =>
      usersApi.create({
        email: email.trim(),
        display_name: displayName.trim(),
        role,
        temporary_password: password,
      }),
    onSuccess: () => {
      toast.success(t('created'));
      void qc.invalidateQueries({ queryKey: qk.users });
      onClose();
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  return (
    <FormDialog
      title={t('newUser')}
      description={t('passwordChangeRequired')}
      error={error}
      busy={create.isPending}
      canSubmit={email.trim() !== '' && displayName.trim() !== '' && password !== ''}
      submitLabel={tc('create')}
      onSubmit={() => create.mutate()}
      onClose={onClose}
    >
      <div>
        <label className="label" htmlFor="nu-name">
          {t('displayName')} *
        </label>
        <input id="nu-name" className="input" value={displayName} onChange={(e) => setDisplayName(e.target.value)} autoFocus />
      </div>
      <div>
        <label className="label" htmlFor="nu-email">
          {t('email')} *
        </label>
        <input id="nu-email" type="email" className="input" value={email} onChange={(e) => setEmail(e.target.value)} />
      </div>
      <div>
        <label className="label" htmlFor="nu-role">
          {t('role')}
        </label>
        <select id="nu-role" className="input" value={role} onChange={(e) => setRole(e.target.value as Role)}>
          {ROLES.map((r) => (
            <option key={r} value={r}>
              {t(`roles.${r}`)}
            </option>
          ))}
        </select>
      </div>
      <PasswordField id="nu-password" label={t('temporaryPassword')} value={password} onChange={setPassword} />
    </FormDialog>
  );
}

function ResetPasswordDialog({ user, onClose }: { user: User; onClose: () => void }) {
  const t = useTranslations('users');
  const ter = useTranslations('errors');
  const toast = useToast();
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);

  const reset = useMutation({
    mutationFn: () => usersApi.resetPassword(user.id, { temporary_password: password }),
    onSuccess: () => {
      toast.success(t('passwordSet'));
      onClose();
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  return (
    <FormDialog
      title={`${t('resetPassword')}: ${user.display_name}`}
      description={t('resetPasswordBody')}
      error={error}
      busy={reset.isPending}
      canSubmit={password !== ''}
      submitLabel={t('resetPassword')}
      onSubmit={() => reset.mutate()}
      onClose={onClose}
    >
      <PasswordField id="rp-password" label={t('newPasswordLabel')} value={password} onChange={setPassword} />
    </FormDialog>
  );
}
