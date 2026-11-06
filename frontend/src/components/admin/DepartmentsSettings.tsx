'use client';

// The workshop's departments and their colours. A department's colour is its mark on every
// technical part, sheet and chip, so it is set here once. Who works in which department is
// set per person under Felhasználók.

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { modulesApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { hasCapability, useAuth } from '@/lib/auth/context';
import { useToast } from '@/components/ui/Toasts';
import { DepartmentChip } from '@/components/orders/ModulesSection';
import type { Department } from '@/lib/api/types';

/** Accents folded, anything else an underscore: "Fényezés" → "fenyezes". */
function keyOf(name: string): string {
  return name
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '');
}

export function DepartmentsSettings() {
  const t = useTranslations('modules');
  const ter = useTranslations('errors');
  const { user } = useAuth();
  const qc = useQueryClient();
  const toast = useToast();
  const query = useQuery({ queryKey: ['departments'], queryFn: () => modulesApi.departments() });
  const [name, setName] = useState('');
  const [color, setColor] = useState('#16a34a');
  const editable = hasCapability(user, 'manage_configuration');

  const refresh = () => void qc.invalidateQueries({ queryKey: ['departments'] });
  const onError = (e: unknown) => toast.error(errorMessage(e, ter, ter('unknownError')));
  const update = useMutation({
    mutationFn: ({ id, body }: { id: number; body: Parameters<typeof modulesApi.updateDepartment>[1] }) =>
      modulesApi.updateDepartment(id, body),
    onSuccess: refresh,
    onError,
  });
  const create = useMutation({
    mutationFn: () =>
      modulesApi.createDepartment({
        key: keyOf(name),
        name: name.trim(),
        color,
        position: ((query.data?.items ?? []).at(-1)?.position ?? 0) + 10,
      }),
    onSuccess: () => {
      setName('');
      refresh();
    },
    onError,
  });

  const items: Department[] = query.data?.items ?? [];
  return (
    <section className="card mt-6">
      <div className="card-header">
        <h2 className="text-section font-semibold">{t('departments')}</h2>
        <p className="text-metadata text-steel-500">{t('departmentsHint')}</p>
      </div>
      <div className="card-content space-y-3">
        <ul className="space-y-2">
          {items.map((d) => (
            <li key={d.id} className="flex flex-wrap items-center gap-3">
              <span className="w-36">
                <DepartmentChip name={d.name} color={d.color} />
              </span>
              <input
                className="input w-48"
                aria-label={`${t('departmentName')}: ${d.name}`}
                defaultValue={d.name}
                disabled={!editable}
                onBlur={(e) => {
                  const v = e.target.value.trim();
                  if (v && v !== d.name) update.mutate({ id: d.id, body: { name: v } });
                }}
              />
              <input
                type="color"
                className="h-9 w-12 cursor-pointer rounded border border-steel-200"
                aria-label={`${t('departmentColor')}: ${d.name}`}
                value={d.color}
                disabled={!editable}
                onChange={(e) => update.mutate({ id: d.id, body: { color: e.target.value } })}
              />
              <label className="flex items-center gap-2 text-body">
                <input
                  type="checkbox"
                  checked={d.is_active}
                  disabled={!editable}
                  onChange={(e) => update.mutate({ id: d.id, body: { is_active: e.target.checked } })}
                />
                {t('departmentActive')}
              </label>
            </li>
          ))}
        </ul>
        {editable && (
          <form
            className="flex flex-wrap items-center gap-3 border-t border-steel-200 pt-3"
            onSubmit={(e) => {
              e.preventDefault();
              if (keyOf(name)) create.mutate();
            }}
          >
            <input
              className="input w-48"
              placeholder={t('departmentNew')}
              aria-label={t('departmentNew')}
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
            <input
              type="color"
              className="h-9 w-12 cursor-pointer rounded border border-steel-200"
              aria-label={t('departmentColor')}
              value={color}
              onChange={(e) => setColor(e.target.value)}
            />
            <button type="submit" className="btn-secondary btn-sm" disabled={!keyOf(name) || create.isPending}>
              {t('add')}
            </button>
          </form>
        )}
      </div>
    </section>
  );
}
