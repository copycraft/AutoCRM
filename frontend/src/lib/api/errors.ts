// Centralised backend error handling.
// Backend contract: { error: { code, message } } with HTTP status (ErrorBody in the OpenAPI document).
// Human-readable messages live in the `errors` catalogue (docs/history/FRONTEND_PLAN.md §14);
// this module only maps codes to catalogue keys. Validation errors surface the
// backend's own message, which carries the field-level detail.

import type { ZodIssue } from 'zod';
import { zErrorBody } from './zod/zod.gen';

export class ApiError extends Error {
  code: string;
  status: number;
  backendMessage: string;

  constructor(code: string, status: number, backendMessage: string) {
    super(backendMessage);
    this.name = 'ApiError';
    this.code = code;
    this.status = status;
    this.backendMessage = backendMessage;
  }
}

/** A response that does not match the generated API contract. */
export class ContractError extends Error {
  readonly endpoint: string;
  readonly issues: ZodIssue[];

  constructor(endpoint: string, issues: ZodIssue[]) {
    super(`Response from ${endpoint} does not match the API contract`);
    this.name = 'ContractError';
    this.endpoint = endpoint;
    this.issues = issues;
    console.error(this.message, issues);
  }
}

/** snake_case backend code → camelCase catalogue key: stage_gate → stageGate. */
export function codeKey(code: string): string {
  return code.replace(/_([a-z])/g, (_, c: string) => c.toUpperCase());
}

export async function parseApiError(res: Response): Promise<ApiError> {
  let body: unknown = null;
  try {
    body = await res.json();
  } catch {
    // non-JSON error body — keep generic message
  }
  const parsed = zErrorBody.safeParse(body);
  const code = parsed.success ? parsed.data.error.code : 'unknown';
  const message = parsed.success ? parsed.data.error.message : `Hiba (${res.status})`;
  return new ApiError(code, res.status, message);
}

/**
 * The database constraint a rejected write tripped, or `null`.
 *
 * `error.rs` puts the constraint name in parentheses at the end of the message for the
 * three codes that carry one — `value violates a data rule (order_specs_compartments_check)`.
 * That name is the only thing in the response that says *which field* was wrong, so a form
 * that drops it has to fall back on "the data violates the database's rules", which tells
 * the user nothing they can act on.
 */
export function constraintName(err: unknown): string | null {
  if (!(err instanceof ApiError)) return null;
  if (!['constraint_violation', 'duplicate', 'invalid_reference'].includes(err.code)) {
    return null;
  }
  return /\(([a-z0-9_]+)\)\s*$/.exec(err.backendMessage)?.[1] ?? null;
}

/**
 * The user-facing text for any error thrown by the API layer.
 * `t` is the `errors` catalogue translator; catalogue lookup falls back to
 * the raw backend message, then to `fallback`. Validation errors always show
 * the backend's message — it carries the field-level reason.
 *
 * Fresh server texts win over the catalogue: `useLookups` refreshes them from
 * `GET /config/lookups` with every fetch, so a reworded message reaches the
 * web without a deploy. The catalogue stays as the cold-start fallback (a
 * login failure happens before the first fetch).
 */
export function errorMessage(
  err: unknown,
  t: (key: string) => string,
  fallback: string,
): string {
  if (err instanceof ContractError) return t('contractViolation');
  if (err instanceof ApiError) {
    if (err.code === 'validation') return err.backendMessage;
    const fresh = serverErrorTexts[err.code];
    if (fresh) return fresh;
    const key = codeKey(err.code);
    const msg = t(key);
    return msg === key ? err.backendMessage : msg;
  }
  return fallback;
}

/**
 * Error texts from the server's lookups (`error_texts`), keyed by code.
 * Refreshed by `useLookups`; empty until the first fetch.
 */
let serverErrorTexts: Record<string, string> = {};

export function setServerErrorTexts(entries: { code: string; text_hu: string }[]): void {
  const next: Record<string, string> = {};
  for (const entry of entries) {
    if (entry.code && entry.text_hu.trim()) next[entry.code] = entry.text_hu;
  }
  serverErrorTexts = next;
}
