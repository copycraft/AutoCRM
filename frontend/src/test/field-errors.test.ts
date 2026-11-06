// Field-level reporting of a rejected write.
//
// The bug this covers: creating an order with a spec value outside its range produced
// "Az adat sérti az adatbázis megkötéseit." — a banner above eleven identical inputs,
// none of them marked, with no way to tell which value was wrong.
import { describe, expect, it } from 'vitest';

import { ApiError } from '@/lib/api/errors';
import { constraintName } from '@/lib/api/errors';
import { fieldErrorText } from '@/components/forms/OrderForm';

describe('the constraint name in a rejected write', () => {
  it('is taken from the message the backend actually sends', () => {
    // Verbatim from POST /api/orders with compartments = 9 (error.rs builds this string).
    const err = new ApiError(
      'constraint_violation',
      422,
      'value violates a data rule (order_specs_compartments_check)',
    );
    expect(constraintName(err)).toBe('order_specs_compartments_check');
  });

  it('reads the other two codes that carry one', () => {
    expect(
      constraintName(
        new ApiError('duplicate', 409, 'a record with these values already exists (orders_number_key)'),
      ),
    ).toBe('orders_number_key');
    expect(
      constraintName(
        new ApiError(
          'invalid_reference',
          422,
          'a referenced record does not exist or is still in use (orders_partner_id_fkey)',
        ),
      ),
    ).toBe('orders_partner_id_fkey');
  });

  it('is null for errors that name no constraint, so the banner still handles them', () => {
    expect(constraintName(new ApiError('validation', 400, 'title is required'))).toBeNull();
    expect(constraintName(new ApiError('stage_gate', 422, 'nem léphet tovább'))).toBeNull();
    expect(constraintName(new Error('offline'))).toBeNull();
    expect(constraintName(null)).toBeNull();
  });
});

describe('the message under an invalid field', () => {
  const tv = (key: string, values?: Record<string, string | number>) =>
    key === 'range' ? `Csak ${values?.min} és ${values?.max} között.` : key;

  it('renders a range token with its bounds', () => {
    expect(fieldErrorText('range:-40:120', tv)).toBe('Csak -40 és 120 között.');
    expect(fieldErrorText('range:1:5', tv)).toBe('Csak 1 és 5 között.');
  });

  it('passes through a sentence the server wrote rather than blanking the field', () => {
    expect(fieldErrorText('a heater rated at nothing', tv)).toBe('a heater rated at nothing');
  });

  it('is empty when there is no error', () => {
    expect(fieldErrorText(undefined, tv)).toBe('');
  });
});
