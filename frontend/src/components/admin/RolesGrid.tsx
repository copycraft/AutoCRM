'use client';

// Szerepkörök: what each role may see and do, one checkbox per role and right. The admin
// role always has everything and is shown but not editable. A tick is saved at once and
// applies to everyone in the role on their next request; per-person extras stay under
// "Jogosultságok" in the user list.

import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { usersApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { useToast } from '@/components/ui/Toasts';
import { LoadingState } from '@/components/ui/LoadingState';
import { ErrorState } from '@/components/ui/ErrorState';
import type { Capability, Role } from '@/lib/api/types';
import { GRANTABLE, ROLES } from './roles';

/** The rights grouped as the grid shows them: seeing first, then doing, then running it. */
const GROUPS: { key: string; caps: Capability[] }[] = [
  {
    key: 'see',
    caps: GRANTABLE.filter((c) => c.startsWith('view_') && c !== 'view_original_images'),
  },
  {
    key: 'do',
    caps: [
      'edit_partners',
      'edit_leads',
      'edit_orders',
      'change_stages',
      'handle_handovers',
      'manage_blockers',
      'upload_media',
      'delete_media',
      'view_original_images',
      'comment',
      'send_email',
      'issue_invoices',
    ],
  },
  {
    key: 'admin',
    caps: ['annul_invoices', 'manage_configuration', 'manage_settings', 'operate_system'],
  },
];

export function RolesGrid() {
  const t = useTranslations('users');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const toast = useToast();
  const query = useQuery({ queryKey: ['roles'], queryFn: () => usersApi.roles() });

  const save = useMutation({
    mutationFn: ({ role, caps }: { role: Role; caps: Capability[] }) => usersApi.setRole(role, caps),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['roles'] });
      // Your own role may have changed: the menu follows.
      void qc.invalidateQueries({ queryKey: qk.me });
    },
    onError: (e) => toast.error(errorMessage(e, ter, ter('unknownError'))),
  });

  if (query.isLoading) return <LoadingState />;
  if (query.isError || !query.data) return <ErrorState error={query.error} onRetry={() => void query.refetch()} />;
  const grid = new Map(query.data.items.map((r) => [r.role, r]));

  const toggle = (role: Role, cap: Capability, on: boolean) => {
    const current = new Set(grid.get(role)?.capabilities ?? []);
    if (on) current.add(cap);
    else current.delete(cap);
    save.mutate({ role, caps: GRANTABLE.filter((c) => current.has(c)) });
  };

  return (
    <section className="mt-10 space-y-3">
      <div>
        <h2 className="text-section font-semibold">{t('rolesTitle')}</h2>
        <p className="max-w-3xl text-metadata text-steel-500">{t('rolesHint')}</p>
      </div>
      <div className="card overflow-x-auto">
        <table className="w-full text-body">
          <thead>
            <tr className="border-b border-steel-200 text-left text-metadata text-steel-500">
              <th className="sticky left-0 bg-panel px-4 py-2 font-medium">{t('right')}</th>
              {ROLES.map((r) => (
                <th key={r} className="px-2 py-2 text-center font-medium">
                  {t(`roles.${r}`)}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {GROUPS.map((g) => (
              <RoleGroup key={g.key} title={t(`rightGroups.${g.key}`)} caps={g.caps}>
                {(cap) =>
                  ROLES.map((r) => {
                    const row = grid.get(r);
                    const checked = r === 'admin' || !!row?.capabilities.includes(cap);
                    const isDefault = !!row?.defaults.includes(cap);
                    return (
                      <td key={r} className="px-2 py-1.5 text-center">
                        <input
                          type="checkbox"
                          aria-label={`${t(`roles.${r}`)}: ${t(`capabilities.${cap}`)}`}
                          title={isDefault ? t('rightDefault') : undefined}
                          checked={checked}
                          disabled={r === 'admin' || !row?.editable || save.isPending}
                          onChange={(e) => toggle(r, cap, e.target.checked)}
                        />
                      </td>
                    );
                  })
                }
              </RoleGroup>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}

function RoleGroup({
  title,
  caps,
  children,
}: {
  title: string;
  caps: Capability[];
  children: (cap: Capability) => React.ReactNode;
}) {
  const t = useTranslations('users');
  return (
    <>
      <tr className="bg-steel-200/40">
        <th colSpan={ROLES.length + 1} className="px-4 py-1.5 text-left text-metadata font-semibold text-steel-900">
          {title}
        </th>
      </tr>
      {caps.map((cap) => (
        <tr key={cap} className="border-b border-steel-200 last:border-0">
          <td className="sticky left-0 bg-panel px-4 py-1.5">{t(`capabilities.${cap}`)}</td>
          {children(cap)}
        </tr>
      ))}
    </>
  );
}
