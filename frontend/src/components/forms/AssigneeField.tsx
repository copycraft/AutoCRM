'use client';

import { useTranslations } from 'next-intl';
import { errorMessage } from '@/lib/api/errors';
import { useAuth } from '@/lib/auth/context';
import { useColleagues } from '@/hooks/useColleagues';

// Assignee picker. The names come from GET /colleagues, which every signed-in user may
// read, so anyone who may assign (the backend checks EditLeads / EditOrders) can pick a
// colleague — not only admins. Inactive accounts are not offered.
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
  const colleagues = useColleagues();

  const raw = value === 'me' || value === 'all' ? value : (value ?? '');
  const shown = !allowEmpty && raw === '' && allowAll ? 'all' : raw;

  return (
    <label className="flex min-w-44 flex-col gap-1">
      <span className="text-metadata font-medium text-steel-500">{label}</span>
      <select
        className="input"
        value={shown}
        disabled={colleagues.isLoading}
        onChange={(e) => {
          const v = e.target.value;
          onChange(v === 'me' ? 'me' : v === 'all' ? 'all' : v === '' ? null : Number(v));
        }}
      >
        {allowAll && <option value="all">{tc('all')}</option>}
        {allowEmpty && <option value="">{tc('unassigned')}</option>}
        {user && <option value="me">{tc('mineOnly')}</option>}
        {colleagues.data?.items
          .filter((u) => u.is_active)
          .map((u) => (
            <option key={u.id} value={u.id}>
              {u.display_name}
            </option>
          ))}
      </select>
      {colleagues.isError && (
        <span className="text-metadata text-steel-900" role="alert">
          {errorMessage(colleagues.error, ter, ter('unknownError'))}
        </span>
      )}
    </label>
  );
}
