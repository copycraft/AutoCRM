'use client';

import { useQuery } from '@tanstack/react-query';
import { useTranslations } from 'next-intl';
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
}: {
  value: number | null | 'all' | 'me';
  onChange: (v: number | null | 'all' | 'me') => void;
  label: string;
  allowAll?: boolean;
}) {
  const { user } = useAuth();
  const tc = useTranslations('common');
  const isAdmin = canAdmin(user);

  const usersQuery = useQuery({
    queryKey: qk.users,
    queryFn: () => usersApi.list(),
    enabled: isAdmin,
    retry: false,
  });

  if (!isAdmin) {
    return (
      <label className="flex min-w-44 flex-col gap-1">
        <span className="text-metadata font-medium text-steel-500">{label}</span>
        <select
          className="input"
          value={String(value)}
          onChange={(e) => {
            const v = e.target.value;
            onChange(v === 'me' ? 'me' : v === 'all' ? 'all' : v === '' ? null : Number(v));
          }}
        >
          {allowAll && <option value="all">{tc('all')}</option>}
          <option value="">{tc('unassigned')}</option>
          {user && <option value="me">{tc('mineOnly')}</option>}
        </select>
      </label>
    );
  }

  return (
    <label className="flex min-w-44 flex-col gap-1">
      <span className="text-metadata font-medium text-steel-500">{label}</span>
      <select
        className="input"
        value={value === 'me' || value === 'all' ? value : (value ?? '')}
        disabled={usersQuery.isLoading}
        onChange={(e) => {
          const v = e.target.value;
          onChange(v === 'me' ? 'me' : v === 'all' ? 'all' : v === '' ? null : Number(v));
        }}
      >
        {allowAll && <option value="all">{tc('all')}</option>}
        <option value="">{tc('unassigned')}</option>
        {user && <option value="me">{tc('mineOnly')}</option>}
        {usersQuery.data?.items
          .filter((u) => u.is_active)
          .map((u) => (
            <option key={u.id} value={u.id}>
              {u.display_name}
            </option>
          ))}
      </select>
    </label>
  );
}
