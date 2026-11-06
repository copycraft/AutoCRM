// The build questionnaire's rules, mirrored from the backend (`domain/questionnaire.rs`):
// which sections a job is asked, given its project type and the answers so far, and which
// required questions are still unanswered. The server decides; this draws the form and the
// convert dialog without a round trip per keystroke.

import type { Answers, Questionnaire, SpecQuestion, SpecSection } from '@/lib/api/types';

/** `false` is an answer; blank text, an empty list or nothing at all is not. */
export function isAnswered(value: unknown): boolean {
  if (value === undefined || value === null) return false;
  if (typeof value === 'string') return value.trim() !== '';
  if (Array.isArray(value)) return value.length > 0;
  return true;
}

function findActive(qn: Questionnaire, key: string): [SpecSection, SpecQuestion] | null {
  for (const s of qn.sections) {
    const q = s.questions.find((x) => x.key === key && x.is_active);
    if (q) return [s, q];
  }
  return null;
}

function applies(section: SpecSection, projectTypeId: number | null): boolean {
  return (
    section.is_active &&
    (section.project_type_ids.length === 0 ||
      (projectTypeId !== null && section.project_type_ids.includes(projectTypeId)))
  );
}

/** Whether the section is asked: its type matches, and its switch (if any) is on in a
 *  section that is itself asked. A chain that loops back is never asked. */
export function isVisible(
  qn: Questionnaire,
  section: SpecSection,
  projectTypeId: number | null,
  answers: Answers,
  seen: Set<number> = new Set(),
): boolean {
  if (!applies(section, projectTypeId) || seen.has(section.id)) return false;
  seen.add(section.id);
  if (!section.shown_when) return true;
  const trigger = findActive(qn, section.shown_when);
  if (!trigger || trigger[1].kind !== 'yes_no') return false;
  return answers[section.shown_when] === true && isVisible(qn, trigger[0], projectTypeId, answers, seen);
}

export function visibleSections(
  qn: Questionnaire | undefined,
  projectTypeId: number | null,
  answers: Answers,
): SpecSection[] {
  if (!qn) return [];
  return qn.sections.filter((s) => isVisible(qn, s, projectTypeId, answers));
}

/** The active questions of the sections asked, in order. */
export function visibleQuestions(
  qn: Questionnaire | undefined,
  projectTypeId: number | null,
  answers: Answers,
): SpecQuestion[] {
  return visibleSections(qn, projectTypeId, answers).flatMap((s) => s.questions.filter((q) => q.is_active));
}

/** The required questions asked and still unanswered: what blocks the conversion. */
export function missingQuestions(
  qn: Questionnaire | undefined,
  projectTypeId: number | null,
  answers: Answers,
): SpecQuestion[] {
  return visibleQuestions(qn, projectTypeId, answers).filter((q) => q.required && !isAnswered(answers[q.key]));
}

/** A typed number as the server reads it ("12,5" from a Hungarian keyboard), or null. */
export function parseNumber(value: unknown): number | null {
  if (typeof value === 'number') return Number.isFinite(value) ? value : null;
  if (typeof value !== 'string' || value.trim() === '') return null;
  const n = Number(value.trim().replace(',', '.'));
  return Number.isFinite(n) ? n : null;
}

/** Why a value would be refused, before sending it: a token for the `validation`
 *  messages (`range:min:max`, `numeric`, `invalidValue`), or null when it is fine. */
export function answerProblem(q: SpecQuestion, value: unknown): string | null {
  if (q.kind !== 'number' || !isAnswered(value)) return null;
  const n = parseNumber(value);
  if (n === null) return 'numeric';
  const min = q.min_value ?? null;
  const max = q.max_value ?? null;
  if ((min !== null && n < min) || (max !== null && n > max)) {
    return min !== null && max !== null ? `range:${min}:${max}` : 'invalidValue';
  }
  return null;
}

/** An answer as one line of text, for read-only views. */
export function formatAnswer(
  q: SpecQuestion,
  value: unknown,
  yes: string,
  no: string,
): string {
  if (!isAnswered(value)) return '—';
  if (q.kind === 'yes_no') return value === true ? yes : no;
  if (Array.isArray(value)) return value.map(String).join(', ');
  const text = typeof value === 'number' ? String(value).replace('.', ',') : String(value);
  return q.unit ? `${text} ${q.unit}` : text;
}
