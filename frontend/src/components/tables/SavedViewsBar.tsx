'use client';

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { applyParams, snapshotParams, useSavedViews } from '@/hooks/useSavedViews';
import { useToast } from '@/components/ui/Toasts';

export const LIST_PARAM_KEYS: Record<string, string[]> = {
  orders: ['q', 'stage', 'partner', 'ptype', 'assignee', 'open', 'sort', 'page'],
  leads: ['q', 'stage', 'tag', 'assignee', 'open', 'sort', 'page'],
  partners: ['q', 'arch', 'sort', 'page'],
  emails: ['q', 'status', 'page'],
  incoming_invoices: ['q', 'bucket', 'page'],
  subscribers: ['q', 'status', 'tag', 'untagged', 'page'],
};

export function SavedViewsBar({ listKey }: { listKey: string }) {
  const t = useTranslations('qol');
  const toast = useToast();
  const { views, save, remove } = useSavedViews(listKey);
  const [name, setName] = useState('');
  const keys = LIST_PARAM_KEYS[listKey] ?? ['q', 'page'];

  return (
    <div className="card">
      <div className="card-content flex flex-wrap items-center gap-2">
        <span className="text-metadata font-medium text-steel-500">{t('savedViews')}</span>
        {views.length === 0 && (
          <span className="text-metadata text-steel-500">{t('savedViewsEmpty')}</span>
        )}
        {views.map((v) => (
          <span
            key={v.name}
            className="inline-flex items-center gap-1 rounded-full border border-steel-200 bg-panel px-2 py-0.5 text-metadata"
          >
            <button
              type="button"
              className="font-medium hover:underline"
              onClick={() => applyParams(v.params)}
              title={t('savedViewApply', { name: v.name })}
            >
              {v.name}
            </button>
            <button
              type="button"
              className="text-steel-500 hover:text-steel-900"
              onClick={() => remove(v.name)}
              aria-label={t('savedViewDelete', { name: v.name })}
            >
              ×
            </button>
          </span>
        ))}
        <span className="ml-auto flex items-center gap-2">
          <input
            className="input w-40 !py-1"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={t('savedViewName')}
            aria-label={t('savedViewName')}
          />
          <button
            type="button"
            className="btn-secondary btn-sm"
            disabled={!name.trim()}
            onClick={() => {
              save(name, snapshotParams(keys));
              setName('');
              toast.success(t('savedViewSaved'));
            }}
          >
            {t('savedViewSave')}
          </button>
        </span>
      </div>
    </div>
  );
}
