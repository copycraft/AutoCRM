// Thin fetch wrapper around the Rust/Axum backend.
// - Base path /api (rewritten to :8080 in dev via next.config.js)
// - Web auth = httpOnly `autocrm_session` cookie → credentials: 'include'
// - JSON in / JSON out
// - PATCH semantics preserved: caller builds exact body (omit = keep, null = clear)
// - Never logs secrets.

import { parseApiError } from './errors';

const BASE = '/api';

interface RequestOptions {
  method?: string;
  body?: unknown;
  search?: Record<string, string | number | boolean | undefined | null>;
  signal?: AbortSignal;
}

function buildUrl(path: string, search?: RequestOptions['search']): string {
  const url = new URL(BASE + path, window.location.origin);
  if (search) {
    for (const [k, v] of Object.entries(search)) {
      if (v === undefined || v === null || v === '') continue;
      url.searchParams.set(k, String(v));
    }
  }
  return url.toString();
}

export async function apiFetch<T>(path: string, opts: RequestOptions = {}): Promise<T> {
  const { method = 'GET', body, search, signal } = opts;
  const res = await fetch(buildUrl(path, search), {
    method,
    credentials: 'include',
    signal,
    headers: body !== undefined ? { 'Content-Type': 'application/json' } : undefined,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  if (!res.ok) throw await parseApiError(res);
  if (res.status === 204) return undefined as T;
  const text = await res.text();
  if (!text) return undefined as T;
  return JSON.parse(text) as T;
}

export const api = {
  get: <T>(path: string, search?: RequestOptions['search'], signal?: AbortSignal) =>
    apiFetch<T>(path, { search, signal }),
  post: <T>(path: string, body?: unknown) => apiFetch<T>(path, { method: 'POST', body }),
  patch: <T>(path: string, body: unknown) => apiFetch<T>(path, { method: 'PATCH', body }),
  put: <T>(path: string, body: unknown) => apiFetch<T>(path, { method: 'PUT', body }),
  del: <T>(path: string) => apiFetch<T>(path, { method: 'DELETE' }),
};
