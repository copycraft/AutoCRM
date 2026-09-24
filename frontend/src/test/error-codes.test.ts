import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import hu from '@/messages/hu.json';
import { codeKey, errorMessage, ApiError } from '@/lib/api/errors';

// The backend's error-code catalog, published in the OpenAPI document as the
// `ErrorCode` schema (backend/src/error.rs::ERROR_CODES). Every code it can send
// must have catalogue text here; otherwise the user sees the raw English message.
const doc = JSON.parse(
  readFileSync(resolve(__dirname, '../../../openapi/openapi.json'), 'utf8'),
) as { components: { schemas: Record<string, { enum?: string[] }> } };
const codes = doc.components.schemas.ErrorCode?.enum ?? [];

describe('error-code catalog', () => {
  it('is published in the OpenAPI document', () => {
    expect(codes.length).toBeGreaterThan(30);
    expect(codes).toContain('stage_gate');
  });

  it('has catalogue text for every backend code', () => {
    const errors = hu.errors as Record<string, string>;
    const missing = codes.filter((c) => !errors[codeKey(c)]);
    expect(missing).toEqual([]);
  });

  it('lists every code in docs/error-codes.md', () => {
    const docs = readFileSync(resolve(__dirname, '../../../docs/error-codes.md'), 'utf8');
    expect(codes.filter((c) => !docs.includes(`| \`${c}\` |`))).toEqual([]);
  });

  it('renders a known code from the catalogue and validation from the backend', () => {
    const t = (key: string) => (hu.errors as Record<string, string>)[key] ?? key;
    expect(errorMessage(new ApiError('currency_locked', 422, 'x'), t, 'fb')).toBe(
      hu.errors.currencyLocked,
    );
    expect(errorMessage(new ApiError('validation', 400, 'title is required'), t, 'fb')).toBe(
      'title is required',
    );
  });
});
