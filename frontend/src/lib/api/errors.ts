// Centralised backend error handling.
// Backend contract: { error: { code, message } } with HTTP status (ErrorBody in the OpenAPI document).
// Human-readable messages live in the `errors` catalogue (FRONTEND_PLAN.md §14);
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
function codeKey(code: string): string {
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
 * The user-facing text for any error thrown by the API layer.
 * `t` is the `errors` catalogue translator; catalogue lookup falls back to
 * the raw backend message, then to `fallback`. Validation errors always show
 * the backend's message — it carries the field-level reason.
 */
export function errorMessage(
  err: unknown,
  t: (key: string) => string,
  fallback: string,
): string {
  if (err instanceof ContractError) return t('contractViolation');
  if (err instanceof ApiError) {
    if (err.code === 'validation') return err.backendMessage;
    const key = codeKey(err.code);
    const msg = t(key);
    return msg === key ? err.backendMessage : msg;
  }
  return fallback;
}
