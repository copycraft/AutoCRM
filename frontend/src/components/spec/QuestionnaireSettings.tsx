'use client';

// Settings → the build questionnaire. Sections of questions per project type; a section can
// open on a yes/no answer ("Hűteni kell?" → Hűtés). Keys and kinds are fixed once created
// (answers are stored under them), and nothing is deleted, only switched off. Every change
// saves on its own, like the departments above.

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { configApi } from '@/lib/api/endpoints';
import { errorMessage } from '@/lib/api/errors';
import { qk } from '@/lib/query/provider';
import { hasCapability, useAuth } from '@/lib/auth/context';
import { useToast } from '@/components/ui/Toasts';
import { useQuestionnaire } from './QuestionnaireView';
import type { Questionnaire, SpecQuestion, SpecSection } from '@/lib/api/types';

const KINDS = ['yes_no', 'number', 'text', 'long_text', 'choice', 'multi_choice'] as const;

/** Accents folded, anything else an underscore: "Hűtött hőmérséklet" → "hutott_homerseklet". */
export function keyOf(label: string): string {
  return label
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^[^a-z]+|_+$/g, '')
    .slice(0, 50);
}

function numberOrNull(s: string): number | null {
  const n = Number(s.trim().replace(',', '.'));
  return s.trim() !== '' && Number.isFinite(n) ? n : null;
}

export function QuestionnaireSettings() {
  const t = useTranslations('spec.settings');
  const ter = useTranslations('errors');
  const { user } = useAuth();
  const qc = useQueryClient();
  const toast = useToast();
  const questionnaire = useQuestionnaire();
  const projectTypes = useQuery({ queryKey: qk.projectTypes, queryFn: () => configApi.projectTypes() });
  const editable = hasCapability(user, 'manage_configuration');
  const [newSection, setNewSection] = useState('');

  const apply = (data: Questionnaire) => qc.setQueryData(qk.questionnaire, data);
  const onError = (e: unknown) => toast.error(errorMessage(e, ter, ter('unknownError')));
  const patchSection = useMutation({
    mutationFn: ({ id, body }: { id: number; body: Parameters<typeof configApi.patchSpecSection>[1] }) =>
      configApi.patchSpecSection(id, body),
    onSuccess: apply,
    onError,
  });
  const patchQuestion = useMutation({
    mutationFn: ({ id, body }: { id: number; body: Parameters<typeof configApi.patchSpecQuestion>[1] }) =>
      configApi.patchSpecQuestion(id, body),
    onSuccess: apply,
    onError,
  });
  const createSection = useMutation({
    mutationFn: () => configApi.createSpecSection({ key: keyOf(newSection), title: newSection.trim() }),
    onSuccess: (data) => {
      apply(data);
      setNewSection('');
    },
    onError,
  });

  const sections = questionnaire.data?.sections ?? [];
  const switches = sections.flatMap((s) =>
    s.questions.filter((q) => q.kind === 'yes_no' && q.is_active).map((q) => ({ section: s, question: q })),
  );
  const types = projectTypes.data?.items ?? [];

  return (
    <section className="card mt-6" data-testid="questionnaire-settings">
      <div className="card-header">
        <h2 className="text-section font-semibold">{t('title')}</h2>
        <p className="text-metadata text-steel-500">{t('hint')}</p>
      </div>
      <div className="card-content space-y-6">
        {sections.map((s) => (
          <SectionEditor
            key={s.id}
            section={s}
            editable={editable}
            switches={switches.filter((x) => x.section.id !== s.id)}
            types={types.map((p) => ({ id: p.id, label: p.label_hu, active: p.is_active }))}
            onPatch={(body) => patchSection.mutate({ id: s.id, body })}
            onPatchQuestion={(id, body) => patchQuestion.mutate({ id, body })}
            onCreated={apply}
          />
        ))}
        {editable && (
          <form
            className="flex flex-wrap items-center gap-3 border-t border-steel-200 pt-4"
            onSubmit={(e) => {
              e.preventDefault();
              if (keyOf(newSection)) createSection.mutate();
            }}
          >
            <input
              className="input w-64"
              placeholder={t('addSection')}
              aria-label={t('addSection')}
              value={newSection}
              onChange={(e) => setNewSection(e.target.value)}
            />
            <button type="submit" className="btn-secondary btn-sm" disabled={!keyOf(newSection) || createSection.isPending}>
              {t('add')}
            </button>
          </form>
        )}
      </div>
    </section>
  );
}

function SectionEditor({
  section: s,
  editable,
  switches,
  types,
  onPatch,
  onPatchQuestion,
  onCreated,
}: {
  section: SpecSection;
  editable: boolean;
  switches: { section: SpecSection; question: SpecQuestion }[];
  types: { id: number; label: string; active: boolean }[];
  onPatch: (body: Parameters<typeof configApi.patchSpecSection>[1]) => void;
  onPatchQuestion: (id: number, body: Parameters<typeof configApi.patchSpecQuestion>[1]) => void;
  onCreated: (data: Questionnaire) => void;
}) {
  const t = useTranslations('spec.settings');
  const ter = useTranslations('errors');
  const toast = useToast();
  const [label, setLabel] = useState('');
  const [kind, setKind] = useState<(typeof KINDS)[number]>('yes_no');
  const [options, setOptions] = useState('');
  const createQuestion = useMutation({
    mutationFn: () =>
      configApi.createSpecQuestion({
        section_id: s.id,
        key: keyOf(label),
        label: label.trim(),
        kind,
        options: kind === 'choice' || kind === 'multi_choice' ? options.split('\n') : [],
      }),
    onSuccess: (data) => {
      onCreated(data);
      setLabel('');
      setOptions('');
    },
    onError: (e) => toast.error(errorMessage(e, ter, ter('unknownError'))),
  });
  const choice = kind === 'choice' || kind === 'multi_choice';

  return (
    <div className={`rounded-lg border border-steel-200 p-4 ${s.is_active ? '' : 'opacity-60'}`} data-section={s.key}>
      <div className="flex flex-wrap items-end gap-3">
        <div>
          <label className="label" htmlFor={`qs-title-${s.id}`}>{t('sectionTitle')}</label>
          <input
            id={`qs-title-${s.id}`}
            className="input w-56"
            defaultValue={s.title}
            disabled={!editable}
            onBlur={(e) => {
              const v = e.target.value.trim();
              if (v && v !== s.title) onPatch({ title: v });
            }}
          />
        </div>
        <span className="pb-2 font-mono text-metadata text-steel-500">{s.key}</span>
        <div>
          <label className="label" htmlFor={`qs-when-${s.id}`}>{t('shownWhen')}</label>
          <select
            id={`qs-when-${s.id}`}
            className="input w-64"
            value={s.shown_when ?? ''}
            disabled={!editable}
            onChange={(e) => onPatch({ shown_when: e.target.value || null })}
          >
            <option value="">{t('always')}</option>
            {switches.map(({ question }) => (
              <option key={question.key} value={question.key}>
                {t('whenYes', { label: question.label })}
              </option>
            ))}
          </select>
        </div>
        <div>
          <label className="label" htmlFor={`qs-pos-${s.id}`}>{t('position')}</label>
          <input
            id={`qs-pos-${s.id}`}
            type="number"
            className="input w-20 font-mono"
            defaultValue={s.position}
            disabled={!editable}
            onBlur={(e) => {
              const v = Number(e.target.value);
              if (Number.isInteger(v) && v !== s.position) onPatch({ position: v });
            }}
          />
        </div>
        <label className="flex items-center gap-2 pb-2 text-body">
          <input
            type="checkbox"
            checked={s.is_active}
            disabled={!editable}
            onChange={(e) => onPatch({ is_active: e.target.checked })}
          />
          {t('active')}
        </label>
      </div>

      <fieldset className="mt-3">
        <legend className="text-metadata font-medium text-steel-500">
          {t('types')}: {s.project_type_ids.length === 0 && <span>{t('allTypes')}</span>}
        </legend>
        <div className="mt-1 flex flex-wrap gap-x-4 gap-y-1">
          {types
            .filter((p) => p.active || s.project_type_ids.includes(p.id))
            .map((p) => (
              <label key={p.id} className="flex items-center gap-2 text-body">
                <input
                  type="checkbox"
                  checked={s.project_type_ids.includes(p.id)}
                  disabled={!editable}
                  onChange={(e) =>
                    onPatch({
                      project_type_ids: e.target.checked
                        ? [...s.project_type_ids, p.id]
                        : s.project_type_ids.filter((x) => x !== p.id),
                    })
                  }
                />
                {p.label}
              </label>
            ))}
        </div>
      </fieldset>

      <ul className="mt-4 space-y-2">
        {s.questions.map((q) => (
          <QuestionEditor key={q.id} question={q} editable={editable} onPatch={(body) => onPatchQuestion(q.id, body)} />
        ))}
      </ul>

      {editable && (
        <form
          className="mt-3 flex flex-wrap items-start gap-3 border-t border-steel-200 pt-3"
          onSubmit={(e) => {
            e.preventDefault();
            if (keyOf(label)) createQuestion.mutate();
          }}
        >
          <input
            className="input w-64"
            placeholder={t('addQuestion')}
            aria-label={t('addQuestion')}
            value={label}
            onChange={(e) => setLabel(e.target.value)}
          />
          <select
            className="input w-44"
            aria-label={t('kind')}
            value={kind}
            onChange={(e) => setKind(e.target.value as (typeof KINDS)[number])}
          >
            {KINDS.map((k) => (
              <option key={k} value={k}>
                {t(`kinds.${k}`)}
              </option>
            ))}
          </select>
          {choice && (
            <textarea
              className="input w-56"
              rows={3}
              placeholder={t('options')}
              aria-label={t('options')}
              value={options}
              onChange={(e) => setOptions(e.target.value)}
            />
          )}
          <button type="submit" className="btn-secondary btn-sm" disabled={!keyOf(label) || createQuestion.isPending}>
            {t('add')}
          </button>
        </form>
      )}
    </div>
  );
}

function QuestionEditor({
  question: q,
  editable,
  onPatch,
}: {
  question: SpecQuestion;
  editable: boolean;
  onPatch: (body: Parameters<typeof configApi.patchSpecQuestion>[1]) => void;
}) {
  const t = useTranslations('spec.settings');
  const id = `qq-${q.id}`;
  const choice = q.kind === 'choice' || q.kind === 'multi_choice';
  return (
    <li className={`rounded border border-steel-200 bg-panel px-3 py-2 ${q.is_active ? '' : 'opacity-60'}`} data-question={q.key}>
      <div className="flex flex-wrap items-center gap-3">
        <input
          className="input w-64"
          aria-label={`${t('questionLabel')}: ${q.label}`}
          defaultValue={q.label}
          disabled={!editable}
          onBlur={(e) => {
            const v = e.target.value.trim();
            if (v && v !== q.label) onPatch({ label: v });
          }}
        />
        <span className="text-metadata text-steel-500">{t(`kinds.${q.kind}`)}</span>
        <span className="font-mono text-metadata text-steel-500">{q.key}</span>
        <label className="flex items-center gap-2 text-body">
          <input
            type="checkbox"
            checked={q.required}
            disabled={!editable}
            onChange={(e) => onPatch({ required: e.target.checked })}
          />
          {t('required')}
        </label>
        <label className="flex items-center gap-2 text-body">
          <input
            type="checkbox"
            checked={q.is_active}
            disabled={!editable}
            onChange={(e) => onPatch({ is_active: e.target.checked })}
          />
          {t('active')}
        </label>
        <input
          type="number"
          className="input w-20 font-mono"
          aria-label={`${t('position')}: ${q.label}`}
          defaultValue={q.position}
          disabled={!editable}
          onBlur={(e) => {
            const v = Number(e.target.value);
            if (Number.isInteger(v) && v !== q.position) onPatch({ position: v });
          }}
        />
      </div>
      <details className="mt-2">
        <summary className="cursor-pointer text-metadata text-steel-500">{t('help')} · {t('unit')}{choice ? ` · ${t('options')}` : ''}</summary>
        <div className="mt-2 flex flex-wrap items-start gap-3">
          <div>
            <label className="label" htmlFor={`${id}-help`}>{t('help')}</label>
            <input
              id={`${id}-help`}
              className="input w-72"
              defaultValue={q.help ?? ''}
              disabled={!editable}
              onBlur={(e) => {
                const v = e.target.value.trim();
                if (v !== (q.help ?? '')) onPatch({ help: v || null });
              }}
            />
          </div>
          {q.kind === 'number' && (
            <>
              <div>
                <label className="label" htmlFor={`${id}-unit`}>{t('unit')}</label>
                <input
                  id={`${id}-unit`}
                  className="input w-24"
                  defaultValue={q.unit ?? ''}
                  disabled={!editable}
                  onBlur={(e) => {
                    const v = e.target.value.trim();
                    if (v !== (q.unit ?? '')) onPatch({ unit: v || null });
                  }}
                />
              </div>
              <div>
                <label className="label" htmlFor={`${id}-min`}>{t('min')}</label>
                <input
                  id={`${id}-min`}
                  inputMode="decimal"
                  className="input w-24 font-mono"
                  defaultValue={q.min_value ?? ''}
                  disabled={!editable}
                  onBlur={(e) => {
                    const v = numberOrNull(e.target.value);
                    if (v !== (q.min_value ?? null)) onPatch({ min_value: v });
                  }}
                />
              </div>
              <div>
                <label className="label" htmlFor={`${id}-max`}>{t('max')}</label>
                <input
                  id={`${id}-max`}
                  inputMode="decimal"
                  className="input w-24 font-mono"
                  defaultValue={q.max_value ?? ''}
                  disabled={!editable}
                  onBlur={(e) => {
                    const v = numberOrNull(e.target.value);
                    if (v !== (q.max_value ?? null)) onPatch({ max_value: v });
                  }}
                />
              </div>
            </>
          )}
          {choice && (
            <div>
              <label className="label" htmlFor={`${id}-options`}>{t('options')}</label>
              <textarea
                id={`${id}-options`}
                className="input w-64"
                rows={Math.max(3, q.options.length)}
                defaultValue={q.options.join('\n')}
                disabled={!editable}
                onBlur={(e) => {
                  const next = e.target.value.split('\n').map((o) => o.trim()).filter(Boolean);
                  if (next.join('\n') !== q.options.join('\n')) onPatch({ options: next });
                }}
              />
            </div>
          )}
        </div>
      </details>
    </li>
  );
}
