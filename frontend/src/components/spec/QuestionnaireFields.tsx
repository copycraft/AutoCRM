'use client';

// The build questionnaire as form fields: the sections asked for this project type and these
// answers, each question drawn by its kind. Answering a switch ("Hűteni kell?") opens or
// closes the sections behind it as you go. The configuration comes from settings
// (`GET /config/questionnaire`), so a new question needs no deploy.
//
// Values are kept as typed (a number stays the string "12,5" until saved); the server
// normalises them. Fields the parent marks as `highlight` are the required ones still
// unanswered: outlined, and said so under the field.

import { useTranslations } from 'next-intl';
import { answerProblem, isAnswered, visibleSections } from '@/lib/questionnaire';
import { fieldErrorText } from '@/components/forms/OrderForm';
import type { Answers, Questionnaire, SpecQuestion, SpecSection } from '@/lib/api/types';

export function QuestionField({
  question: q,
  value,
  onChange,
  highlight,
  disabled,
  idPrefix,
}: {
  question: SpecQuestion;
  value: unknown;
  onChange: (value: unknown) => void;
  highlight?: boolean;
  disabled?: boolean;
  idPrefix: string;
}) {
  const t = useTranslations('spec');
  const tc = useTranslations('common');
  const tv = useTranslations('validation');
  const id = `${idPrefix}-${q.key}`;
  const problem = answerProblem(q, value);
  const flagged = highlight && !isAnswered(value);
  const describedBy = problem || flagged || q.help ? `${id}-note` : undefined;
  const wide = q.kind === 'long_text' || q.kind === 'multi_choice';
  const outline = flagged || problem ? 'ring-2 ring-signal/60' : '';

  let control: React.ReactNode;
  switch (q.kind) {
    case 'yes_no':
      control = (
        <div
          role="radiogroup"
          aria-labelledby={`${id}-label`}
          aria-describedby={describedBy}
          className={`inline-flex rounded-lg border border-steel-200 ${outline}`}
        >
          {[true, false].map((choice) => (
            <button
              key={String(choice)}
              type="button"
              role="radio"
              aria-checked={value === choice}
              disabled={disabled}
              onClick={() => onChange(value === choice ? null : choice)}
              className={`px-4 py-1.5 text-body first:rounded-l-lg last:rounded-r-lg ${
                value === choice ? 'bg-steel-900 text-surface' : 'bg-surface text-steel-900 hover:bg-panel'
              }`}
            >
              {choice ? tc('yes') : tc('no')}
            </button>
          ))}
        </div>
      );
      break;
    case 'number':
      control = (
        <div className="flex items-center gap-2">
          <input
            id={id}
            inputMode="decimal"
            className={`input font-mono ${outline}`}
            value={value == null ? '' : String(value)}
            disabled={disabled}
            aria-invalid={problem ? true : undefined}
            aria-describedby={describedBy}
            onChange={(e) => onChange(e.target.value)}
          />
          {q.unit && <span className="shrink-0 text-metadata text-steel-500">{q.unit}</span>}
        </div>
      );
      break;
    case 'long_text':
      control = (
        <textarea
          id={id}
          rows={3}
          className={`input ${outline}`}
          value={typeof value === 'string' ? value : ''}
          disabled={disabled}
          aria-describedby={describedBy}
          onChange={(e) => onChange(e.target.value)}
        />
      );
      break;
    case 'choice':
      control = (
        <select
          id={id}
          className={`input ${outline}`}
          value={typeof value === 'string' ? value : ''}
          disabled={disabled}
          aria-describedby={describedBy}
          onChange={(e) => onChange(e.target.value || null)}
        >
          <option value="">—</option>
          {/* A choice renamed since keeps showing on the record that has it. */}
          {typeof value === 'string' && value && !q.options.includes(value) && (
            <option value={value}>{value}</option>
          )}
          {q.options.map((o) => (
            <option key={o} value={o}>
              {o}
            </option>
          ))}
        </select>
      );
      break;
    case 'multi_choice': {
      const picked = Array.isArray(value) ? value.map(String) : [];
      control = (
        <div
          role="group"
          aria-labelledby={`${id}-label`}
          aria-describedby={describedBy}
          className={`flex flex-wrap gap-x-4 gap-y-1 rounded-lg ${outline}`}
        >
          {[...q.options, ...picked.filter((p) => !q.options.includes(p))].map((o) => (
            <label key={o} className="flex items-center gap-2 text-body">
              <input
                type="checkbox"
                className="h-4 w-4"
                checked={picked.includes(o)}
                disabled={disabled}
                onChange={(e) =>
                  onChange(e.target.checked ? [...picked, o] : picked.filter((p) => p !== o))
                }
              />
              {o}
            </label>
          ))}
        </div>
      );
      break;
    }
    default:
      control = (
        <input
          id={id}
          className={`input ${outline}`}
          value={typeof value === 'string' ? value : ''}
          disabled={disabled}
          aria-describedby={describedBy}
          onChange={(e) => onChange(e.target.value)}
        />
      );
  }

  return (
    <div className={wide ? 'md:col-span-2' : undefined} data-question={q.key}>
      <label id={`${id}-label`} className="label" htmlFor={q.kind === 'yes_no' || q.kind === 'multi_choice' ? undefined : id}>
        {q.label}
        {q.required && (
          <span className="ml-1 text-signal" title={t('requiredHint')} aria-label={t('requiredHint')}>
            *
          </span>
        )}
      </label>
      {control}
      {(problem || flagged || q.help) && (
        <p
          id={`${id}-note`}
          className={`mt-1 text-metadata ${problem || flagged ? 'text-signal' : 'text-steel-500'}`}
          role={problem ? 'alert' : undefined}
        >
          {problem ? fieldErrorText(problem, tv) : flagged ? t('requiredHint') : q.help}
        </p>
      )}
    </div>
  );
}

function SectionFields({
  section,
  answers,
  onChange,
  highlight,
  disabled,
  exclude,
  idPrefix,
}: {
  section: SpecSection;
  answers: Answers;
  onChange: (key: string, value: unknown) => void;
  highlight?: ReadonlySet<string>;
  disabled?: boolean;
  exclude?: ReadonlySet<string>;
  idPrefix: string;
}) {
  const questions = section.questions.filter((q) => q.is_active && !exclude?.has(q.key));
  if (questions.length === 0) return null;
  return (
    <fieldset className="border-t border-steel-200 pt-4 first:border-t-0 first:pt-0">
      <legend className="mb-3 text-body font-semibold text-steel-900">{section.title}</legend>
      <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
        {questions.map((q) => (
          <QuestionField
            key={q.key}
            question={q}
            value={answers[q.key]}
            onChange={(v) => onChange(q.key, v)}
            highlight={highlight?.has(q.key)}
            disabled={disabled}
            idPrefix={idPrefix}
          />
        ))}
      </div>
    </fieldset>
  );
}

/** Every section asked, as fields. `exclude` leaves out questions drawn elsewhere (the
 *  convert dialog's missing block). */
export function QuestionnaireFields({
  questionnaire,
  projectTypeId,
  answers,
  onChange,
  highlight,
  disabled,
  exclude,
  idPrefix = 'q',
}: {
  questionnaire: Questionnaire | undefined;
  projectTypeId: number | null;
  answers: Answers;
  onChange: (next: Answers) => void;
  highlight?: ReadonlySet<string>;
  disabled?: boolean;
  exclude?: ReadonlySet<string>;
  idPrefix?: string;
}) {
  const sections = visibleSections(questionnaire, projectTypeId, answers);
  const set = (key: string, value: unknown) => {
    const next = { ...answers };
    if (value === null || value === undefined) delete next[key];
    else next[key] = value;
    onChange(next);
  };
  return (
    <div className="space-y-4">
      {sections.map((s) => (
        <SectionFields
          key={s.id}
          section={s}
          answers={answers}
          onChange={set}
          highlight={highlight}
          disabled={disabled}
          exclude={exclude}
          idPrefix={idPrefix}
        />
      ))}
    </div>
  );
}
