// Centralised backend error handling.
// Backend contract: { error: { code, message } } with HTTP status (ErrorBody in the OpenAPI document).
// Maps codes to Hungarian human-readable messages (FRONTEND_PLAN.md §14).

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

const HUNGARIAN_MESSAGES: Record<string, string> = {
  validation: 'Érvényesítési hiba. Ellenőrizze a megadott adatokat.',
  unauthenticated: 'Nincs bejelentkezve. Kérjük, jelentkezzen be újra.',
  forbidden: 'Nincs jogosultsága ehhez a művelethez.',
  not_found: 'A kért adat nem található.',
  duplicate: 'Ilyen adat már létezik.',
  immutable: 'Ez a kép bizonyítási célból védett, ezért nem törölhető.',
  already_converted: 'Ez a lead már megrendeléssé lett alakítva.',
  already_resolved: 'Ez az akadály már meg van oldva.',
  not_retryable: 'Ez a művelet nem próbálható újra.',
  stage_gate: 'A megrendelés nem léphet tovább, mert egy szükséges feltétel még nem teljesült.',
  note_required: 'A visszalépéshez indoklás szükséges.',
  invalid_transition: 'Ez a fázisváltás nem engedélyezett.',
  use_conversion: 'Használja az átalakítás funkciót a won fázis eléréséhez.',
  lead_converted: 'Ez a lead már megrendeléssé lett alakítva.',
  currency_locked: 'A pénznem nem módosítható, amíg a megrendelésnek vannak tételei.',
  password_change_required: 'Első bejelentkezéskor kötelező jelszót változtatni.',
  upload_missing: 'A feltöltés nem található vagy lejárt. Kezdje újra.',
  upload_mismatch: 'A feltöltött fájl nem egyezik a bejelentettel. Ellenőrizze a fájlt.',
  invalid_ticket: 'Érvénytelen vagy lejárt feltöltési jegy.',
  last_admin: 'Az utolsó aktív adminisztrátor nem tiltható le.',
  invalid_reference: 'Érvénytelen hivatkozás.',
  constraint_violation: 'Az adat sérti az adatbázis megkötéseit.',
  too_many_requests: 'Túl sok sikertelen próbálkozás. A fiók 15 percre zárolva.',
  contract_violation: 'A szerver válasza nem a várt formátumú.',
};

export function hungarianMessage(code: string, fallback: string): string {
  return HUNGARIAN_MESSAGES[code] ?? fallback;
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
  return new ApiError(code, res.status, hungarianMessage(code, message));
}

export function isApiError(err: unknown): err is ApiError {
  return err instanceof ApiError;
}

/** The user-facing text for any error thrown by the API layer. */
export function errorMessage(err: unknown, fallback: string): string {
  if (err instanceof ApiError) return err.backendMessage;
  if (err instanceof ContractError) return hungarianMessage('contract_violation', fallback);
  return fallback;
}
