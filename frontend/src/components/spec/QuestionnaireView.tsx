'use client';

// The build questionnaire's answers, read-only and grouped by the sections asked. Unanswered
// questions show a dash; required ones still unanswered are counted above, since those are
// what stand between a lead and its order.

import { useQuery } from '@tanstack/react-query';
import { useTranslations } from 'next-intl';
import { configApi } from '@/lib/api/endpoints';
import { qk } from '@/lib/query/provider';
import { formatAnswer, missingQuestions, visibleSections } from '@/lib/questionnaire';
import type { Answers, Questionnaire, SpecQuestion } from '@/lib/api/types';

export function useQuestionnaire() {
  return useQuery({
    queryKey: qk.questionnaire,
    queryFn: () => configApi.questionnaire(),
    staleTime: 5 * 60_000,
  });
}

export function MissingBadge({
  questionnaire,
  projectTypeId,
  answers,
}: {
  questionnaire: Questionnaire | undefined;
  projectTypeId: number | null;
  answers: Answers;
}) {
  const t = useTranslations('spec');
  if (!questionnaire) return null;
  const missing = missingQuestions(questionnaire, projectTypeId, answers).length;
  return missing > 0 ? (
    <span className="badge-signal">{t('missingCount', { count: missing })}</span>
  ) : (
    <span className="badge bg-done/10 text-done">{t('complete')}</span>
  );
}

export function QuestionnaireView({
  questionnaire,
  projectTypeId,
  answers,
  renderAnswer,
}: {
  questionnaire: Questionnaire | undefined;
  projectTypeId: number | null;
  answers: Answers;
  /** Draws one answer differently (the cooling serial, editable from the phone and here). */
  renderAnswer?: (question: SpecQuestion, value: unknown) => React.ReactNode | undefined;
}) {
  const t = useTranslations('spec');
  const tc = useTranslations('common');
  const sections = visibleSections(questionnaire, projectTypeId, answers);
  if (sections.length === 0) {
    return <p className="text-body text-steel-500">{t('empty')}</p>;
  }
  return (
    <div className="space-y-4">
      {sections.map((s) => {
        const questions = s.questions.filter((q) => q.is_active);
        if (questions.length === 0) return null;
        return (
          <div key={s.id}>
            <h3 className="text-body font-semibold text-steel-900">{s.title}</h3>
            <dl className="mt-2 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
              {questions.map((q) => {
                const custom = renderAnswer?.(q, answers[q.key]);
                if (custom !== undefined) return <div key={q.key}>{custom}</div>;
                return (
                  <div
                    key={q.key}
                    className={`flex flex-col gap-0.5 ${q.kind === 'long_text' ? 'sm:col-span-2 lg:col-span-3' : ''}`}
                  >
                    <dt className="text-metadata font-medium text-steel-500">{q.label}</dt>
                    <dd
                      className={`text-body ${q.kind === 'number' ? 'font-mono' : ''} ${
                        q.kind === 'long_text' ? 'whitespace-pre-wrap' : ''
                      }`}
                    >
                      {formatAnswer(q, answers[q.key], tc('yes'), tc('no'))}
                    </dd>
                  </div>
                );
              })}
            </dl>
          </div>
        );
      })}
    </div>
  );
}
