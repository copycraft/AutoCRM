'use client';

// The build questionnaire on a lead: the project type and the answers, filled while
// selling. Saving never demands completeness; the badge says how many required answers the
// conversion will still ask for, and the convert dialog asks exactly those.

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { configApi, leadsApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { qk } from '@/lib/query/provider';
import { useToast } from '@/components/ui/Toasts';
import { QuestionnaireFields } from './QuestionnaireFields';
import { MissingBadge, QuestionnaireView, useQuestionnaire } from './QuestionnaireView';
import type { Answers, LeadSpec } from '@/lib/api/types';

export function ProjectTypeSelect({
  id,
  value,
  onChange,
  disabled,
}: {
  id: string;
  value: number | null;
  onChange: (id: number | null) => void;
  disabled?: boolean;
}) {
  const ter = useTranslations('errors');
  const projectTypes = useQuery({ queryKey: qk.projectTypes, queryFn: () => configApi.projectTypes() });
  const items = projectTypes.data?.items ?? [];
  return (
    <>
      <select
        id={id}
        className="input"
        value={value ?? ''}
        disabled={disabled || projectTypes.isLoading}
        onChange={(e) => onChange(e.target.value ? Number(e.target.value) : null)}
      >
        <option value="">—</option>
        {items
          .filter((p) => p.is_active || p.id === value)
          .map((p) => (
            <option key={p.id} value={p.id}>
              {p.label_hu}
            </option>
          ))}
      </select>
      {projectTypes.isError && (
        <p className="mt-1 text-metadata text-signal" role="alert">
          {errorMessage(projectTypes.error, ter, ter('unknownError'))}
        </p>
      )}
    </>
  );
}

export function useProjectTypeLabel(id: number | null | undefined): string {
  const projectTypes = useQuery({ queryKey: qk.projectTypes, queryFn: () => configApi.projectTypes() });
  if (id == null) return '—';
  return projectTypes.data?.items.find((p) => p.id === id)?.label_hu ?? `#${id}`;
}

export function LeadSpecSection({
  leadId,
  spec,
  editable,
}: {
  leadId: number;
  spec: LeadSpec;
  editable: boolean;
}) {
  const t = useTranslations('spec');
  const tc = useTranslations('common');
  const ter = useTranslations('errors');
  const qc = useQueryClient();
  const toast = useToast();
  const questionnaire = useQuestionnaire();
  const [editing, setEditing] = useState(false);
  const [projectTypeId, setProjectTypeId] = useState<number | null>(spec.project_type_id ?? null);
  const [answers, setAnswers] = useState<Answers>(spec.answers);
  const [error, setError] = useState<string | null>(null);
  const typeLabel = useProjectTypeLabel(spec.project_type_id);

  const save = useMutation({
    mutationFn: () => leadsApi.putSpec(leadId, { project_type_id: projectTypeId, answers }),
    onSuccess: (saved) => {
      setEditing(false);
      setError(null);
      setProjectTypeId(saved.project_type_id ?? null);
      setAnswers(saved.answers);
      toast.success(t('saved'));
      void qc.invalidateQueries({ queryKey: qk.lead(leadId) });
    },
    onError: (e) => setError(errorMessage(e, ter, ter('unknownError'))),
  });

  const start = () => {
    setProjectTypeId(spec.project_type_id ?? null);
    setAnswers(spec.answers);
    setError(null);
    setEditing(true);
  };

  return (
    <section className="border-t border-steel-200 pt-5 first:border-t-0 first:pt-0" data-testid="lead-spec">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-wrap items-center gap-3">
          <h2 className="text-section font-semibold">{t('title')}</h2>
          <MissingBadge
            questionnaire={questionnaire.data}
            projectTypeId={editing ? projectTypeId : (spec.project_type_id ?? null)}
            answers={editing ? answers : spec.answers}
          />
        </div>
        {editable && !editing && (
          <button className="btn-secondary btn-sm" onClick={start}>
            {t('edit')}
          </button>
        )}
      </div>
      <p className="mt-1 text-metadata text-steel-500">{t('explanation')}</p>

      {editing ? (
        <form
          className="mt-4 space-y-4"
          onSubmit={(e) => {
            e.preventDefault();
            save.mutate();
          }}
        >
          <div className="max-w-sm">
            <label className="label" htmlFor="ls-type">
              {t('projectType')}
            </label>
            <ProjectTypeSelect id="ls-type" value={projectTypeId} onChange={setProjectTypeId} />
            {projectTypeId === null && <p className="mt-1 text-metadata text-steel-500">{t('chooseType')}</p>}
          </div>
          <QuestionnaireFields
            questionnaire={questionnaire.data}
            projectTypeId={projectTypeId}
            answers={answers}
            onChange={setAnswers}
            idPrefix="ls"
          />
          {error && (
            <p className="rounded-lg bg-steel-200/50 px-3 py-2 text-body text-steel-900" role="alert">
              {error}
            </p>
          )}
          <div className="flex justify-end gap-2">
            <button type="button" className="btn-ghost" onClick={() => setEditing(false)}>
              {tc('cancel')}
            </button>
            <button type="submit" className="btn-primary" disabled={save.isPending}>
              {save.isPending ? tc('saving') : t('save')}
            </button>
          </div>
        </form>
      ) : (
        <div className="mt-4 space-y-4">
          <div className="flex flex-col gap-0.5">
            <span className="text-metadata font-medium text-steel-500">{t('projectType')}</span>
            <span className="text-body">{typeLabel}</span>
          </div>
          <QuestionnaireView
            questionnaire={questionnaire.data}
            projectTypeId={spec.project_type_id ?? null}
            answers={spec.answers}
          />
        </div>
      )}
    </section>
  );
}
