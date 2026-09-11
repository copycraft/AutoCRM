// Thin fetch wrapper around the Rust/Axum backend.
// - Base path /api (rewritten to the backend in dev via next.config.js)
// - Web auth = httpOnly `autocrm_session` cookie → credentials: 'include'
// - Every JSON response is validated at this boundary against the zod schema generated from
//   the API contract; a mismatch throws ContractError instead of flowing on as a lie.
// - PATCH semantics preserved: caller builds exact body (omit = keep, null = clear)
// - Never logs secrets.

import type { ZodType, ZodTypeDef } from 'zod';
import { ContractError, parseApiError } from './errors';

const BASE = '/api';

type Search = Record<string, string | number | boolean | undefined | null>;

interface RequestOptions {
  method?: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';
  body?: unknown;
  search?: Search;
  signal?: AbortSignal;
}

function buildUrl(path: string, search?: Search): string {
  const url = new URL(BASE + path, window.location.origin);
  if (search) {
    for (const [k, v] of Object.entries(search)) {
      if (v === undefined || v === null || v === '') continue;
      url.searchParams.set(k, String(v));
    }
  }
  return url.toString();
}

async function send(path: string, opts: RequestOptions): Promise<Response> {
  const { method = 'GET', body, search, signal } = opts;
  const res = await fetch(buildUrl(path, search), {
    method,
    credentials: 'include',
    signal,
    headers: body !== undefined ? { 'Content-Type': 'application/json' } : undefined,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  if (!res.ok) throw await parseApiError(res);
  return res;
}

/** A JSON response, parsed and validated against its generated schema. */
export async function request<T>(
  path: string,
  schema: ZodType<T, ZodTypeDef, unknown>,
  opts: RequestOptions = {},
): Promise<T> {
  const res = await send(path, opts);
  const json: unknown = await res.json();
  const parsed = schema.safeParse(json);
  if (!parsed.success) {
    throw new ContractError(`${opts.method ?? 'GET'} ${path}`, parsed.error.issues);
  }
  return parsed.data;
}

/** A response without a body (204 No Content, 202 Accepted). */
export async function requestNoContent(path: string, opts: RequestOptions = {}): Promise<void> {
  await send(path, opts);
}
