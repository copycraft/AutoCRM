// Sales journey (SALES-L): a lead's contact must belong to the lead's partner.
//
// The backend refuses a contact without (or from another) partner on create
// (service/leads.rs: "contact belongs to a different partner"). The form must not send
// a stale contact id after the partner was cleared.
import { describe, expect, it } from 'vitest';

import { leadCreateBody, leadPatchBody, type LeadFormValues } from '@/components/forms/LeadForm';
import type { Lead } from '@/lib/api/types';

const base: LeadFormValues = {
  title: 'Hűtős Sprinter',
  partner: null,
  contact_id: '',
  contact_name: '',
  contact_email: '',
  contact_phone: '',
  source: '',
  description: '',
  assigned_to: null,
  quoted_value: '',
  currency: '',
  quote_valid_until: '',
};

const original = {
  id: 7,
  title: 'Hűtős Sprinter',
  partner_id: 1,
  contact_id: 5,
  contact_name: null,
  contact_email: null,
  contact_phone: null,
  source: null,
  description: null,
  assigned_to: null,
  quoted_value_minor: null,
  currency: null,
  quote_valid_until: null,
  created_by: 1,
  minicrm_id: null,
  created_at: '2026-09-01T08:00:00Z',
  updated_at: '2026-09-01T08:00:00Z',
} as Lead;

describe('a lead contact without a partner', () => {
  it('is not sent on create when no partner is chosen', () => {
    const body = leadCreateBody({ ...base, partner: null, contact_id: '5' }, undefined);
    expect(body.partner_id).toBeNull();
    expect(body.contact_id).toBeNull();
  });

  it('is cleared on edit when the partner is removed', () => {
    const body = leadPatchBody(original, { ...base, partner: null, contact_id: '5' }, undefined);
    expect(body.partner_id).toBeNull();
    expect(body.contact_id).toBeNull();
  });

  it('is kept when the partner stays', () => {
    const body = leadPatchBody(original, { ...base, partner: { id: 1, name: 'P' }, contact_id: '5' }, undefined);
    expect(body).not.toHaveProperty('contact_id');
  });
});
