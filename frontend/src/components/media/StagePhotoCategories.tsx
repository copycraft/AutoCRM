'use client';

// Which category a new photo takes while an order is in each stage. In MEO it is the
// completion photo the MEO gate asks for; the phone and the web preselect it.

import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { configApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { errorMessage } from '@/lib/api/errors';
import { useLookups } from '@/hooks/useLookups';
import { useToast } from '@/components/ui/Toasts';
import type { ImageCategory } from '@/lib/api/types';

export function StagePhotoCategories() {
  const t = useTranslations('media');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const { toast } = useToast();
  const { data: lookups } = useLookups();
  const list = useQuery({ queryKey: qk.stagePhotoCategories, queryFn: () => configApi.stagePhotoCategories() });
  const save = useMutation({
    mutationFn: ({ key, category }: { key: string; category: ImageCategory | null }) =>
      configApi.setStagePhotoCategory(key, { category }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: qk.stagePhotoCategories });
      void qc.invalidateQueries({ queryKey: ['order'] });
    },
    onError: (e) => toast('error', errorMessage(e, ter, ter('unknownError'))),
  });
  const choices = (lookups?.image_categories ?? []).filter((c) => !c.immutable && c.key !== 'inspection');
  return (
    <section className="card">
      <div className="card-header">
        <h2 className="text-section font-semibold">{t('stageCategoriesTitle')}</h2>
        <p className="text-metadata text-steel-500">{t('stageCategoriesIntro')}</p>
      </div>
      <div className="card-content">
        <ul className="divide-y divide-steel-200">
          {(list.data?.items ?? []).map((s) => (
            <li key={s.stage_key} className="flex items-center justify-between gap-3 py-2">
              <span className="text-body">{s.label_hu}</span>
              <select
                className="input h-8 w-56 py-0"
                value={s.category ?? ''}
                onChange={(e) => save.mutate({ key: s.stage_key, category: (e.target.value || null) as ImageCategory | null })}
                aria-label={s.label_hu}
              >
                <option value="">{t('noStageDefault')}</option>
                {choices.map((c) => (
                  <option key={c.key} value={c.key}>{c.label_hu}</option>
                ))}
              </select>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
