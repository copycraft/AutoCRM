'use client';

import { useQuery } from '@tanstack/react-query';
import { useTranslations } from 'next-intl';
import { errorMessage } from '@/lib/api/errors';
import { usersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { canAdmin, useAuth } from '@/lib/auth/context';

// Assignee picker that respects the backend capability model:
// GET /users is admin-only (ManageUsers), so non-admins get a
// "me / unassigned" choice instead of a user dropdown. No fake data.
export function AssigneeField({
  value,
  onChange,
  label,
  allowAll,
  allowEmpty = true,
}: {
  value: number | null | 'all' | 'me';
  onChange: (v: number | null | 'all' | 'me') => void;
  label: string;
  allowAll?: boolean;
  /**
   * Filters pass false: the empty option used to read "unassigned" while
   * applying no filter at all. Without it, "all" is the explicit no-filter
   * choice and null displays as such.
   */
  allowEmpty?: boolean;
}) {
  const { user } = useAuth();
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const isAdmin = canAdmin(user);

  const usersQuery = useQuery({
    queryKey: qk.users,
    queryFn: () => usersApi.list(),
    enabled: isAdmin,
    retry: false,
  });

  if (!isAdmin) {
    const rawValue = value === null ? '' : String(value);
    return (
      <label className="flex min-w-44 flex-col gap-1">
        <span className="text-metadata font-medium text-steel-500">{label}</span>
        <select
          className="input"
          value={!allowEmpty && rawValue === '' && allowAll ? 'all' : rawValue}
          onChange={(e) => {
            const v = e.target.value;
            onChange(v === 'me' ? 'me' : v === 'all' ? 'all' : v === '' ? null : Number(v));
          }}
        >
          {allowAll && <option value="all">{tc('all')}</option>}
          {allowEmpty && <option value="">{tc('unassigned')}</option>}
          {user && <option value="me">{tc('mineOnly')}</option>}
        </select>
      </label>
    );
  }

  const raw = value === 'me' || value === 'all' ? value : (value ?? '');
  const shown = !allowEmpty && raw === '' && allowAll ? 'all' : raw;

  return (
    <label className="flex min-w-44 flex-col gap-1">
      <span className="text-metadata font-medium text-steel-500">{label}</span>
      <select
        className="input"
        value={shown}
        disabled={usersQuery.isLoading}
        onChange={(e) => {
          const v = e.target.value;
          onChange(v === 'me' ? 'me' : v === 'all' ? 'all' : v === '' ? null : Number(v));
        }}
      >
        {allowAll && <option value="all">{tc('all')}</option>}
        {allowEmpty && <option value="">{tc('unassigned')}</option>}
        {user && <option value="me">{tc('mineOnly')}</option>}
        {usersQuery.data?.items
          .filter((u) => u.is_active)
          .map((u) => (
            <option key={u.id} value={u.id}>
              {u.display_name}
            </option>
          ))}
      </select>
      {usersQuery.isError && (
        <span className="text-xs text-steel-900" role="alert">
          {errorMessage(usersQuery.error, ter, ter('unknownError'))}
        </span>
      )}
    </label>
  );
}
