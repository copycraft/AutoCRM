// Newsletter audience (MAIL-L5): the office must see who "sent" really meant.
//
// Suppressed addresses never make the BCC list, so the pre-send count must not
// include them; and the toast after sending must carry the server's recipient count,
// not the generic "queued" note.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, screen, waitFor } from '@testing-library/react';
import { renderPage } from './harness';
import { overrides, resetOverrides } from './client-mock';
import { ComposeForm } from '@/components/email/ComposeForm';

vi.mock('@/lib/api/client', () => import('./client-mock'));

afterEach(() => resetOverrides());

function subscriptions() {
  return {
    items: [
      { id: 1, email: 'a@example.hu', name: 'A', unsubscribed_at: null },
      { id: 2, email: 'b@example.hu', name: 'B', unsubscribed_at: null },
      { id: 3, email: 'c@example.hu', name: 'C', unsubscribed_at: null },
      { id: 4, email: 'd@example.hu', name: 'D', unsubscribed_at: '2026-09-20T10:00:00Z' },
    ],
  };
}

describe('newsletter audience', () => {
  it('excludes unsubscribed and suppressed addresses from the pre-send count', async () => {
    overrides.set('/newsletter/subscriptions', subscriptions);
    overrides.set('/email-suppressions', () => ({
      items: [{ email: 'B@EXAMPLE.HU', reason: null, created_at: '2026-09-20T10:00:00Z' }],
    }));
    renderPage(<ComposeForm defaultAudience="newsletter" onSent={() => {}} />);
    // a and c only: d opted out, b is suppressed (matched case-insensitively).
    await screen.findByText('2 feliratkozó kapja meg BCC-ben.');
  });

  it('counts every active subscriber when nothing is suppressed', async () => {
    overrides.set('/newsletter/subscriptions', subscriptions);
    overrides.set('/email-suppressions', () => ({ items: [] }));
    renderPage(<ComposeForm defaultAudience="newsletter" onSent={() => {}} />);
    await screen.findByText('3 feliratkozó kapja meg BCC-ben.');
  });

  it('hands the server recipient count to onSent instead of just the email id', async () => {
    overrides.set('/newsletter/subscriptions', subscriptions);
    overrides.set('/email-suppressions', () => ({ items: [] }));
    overrides.set('/emails/preview', () => ({
      subject: 'Akció',
      body_html: '<p>Akció</p>',
      unresolved: [],
      recipient_suppressed: false,
    }));
    overrides.set('/newsletter/send', () => ({ email_id: 99, recipients: 3 }));
    const seen: { id: number; recipients?: number }[] = [];
    renderPage(
      <ComposeForm
        defaultAudience="newsletter"
        onSent={(id, recipients) => {
          seen.push({ id, recipients });
        }}
      />,
    );
    fireEvent.change(await screen.findByLabelText('Tárgy'), { target: { value: 'Akció' } });
    fireEvent.change(screen.getByLabelText('Szöveg'), { target: { value: 'Olcsó hűtő' } });
    fireEvent.click(await screen.findByRole('button', { name: 'Hírlevél kiküldése' }));
    await waitFor(() => expect(seen).toEqual([{ id: 99, recipients: 3 }]));
  });
});
