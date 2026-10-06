// Newsletter audience (MAIL-L5): the office must see who "sent" really meant.
//
// The pre-send count comes from the server's audience query, the same one the send uses
// (active, confirmed, not suppressed, and with a chosen tag when tags are picked), and
// the toast after sending must carry the server's recipient count.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, screen, waitFor } from '@testing-library/react';
import { renderPage } from './harness';
import { overrides, resetOverrides } from './client-mock';
import { ComposeForm } from '@/components/email/ComposeForm';

vi.mock('@/lib/api/client', () => import('./client-mock'));

afterEach(() => resetOverrides());

describe('newsletter audience', () => {
  it('shows the server count for everyone when no tag is picked', async () => {
    const asked: unknown[] = [];
    overrides.set('/newsletter/audience', (search) => {
      asked.push(search?.tags);
      return { recipients: 2 };
    });
    renderPage(<ComposeForm defaultAudience="newsletter" onSent={() => {}} />);
    await screen.findByText('2 feliratkozó kapja meg BCC-ben.');
    expect(asked).toEqual([undefined]);
  });

  it('asks for the audience of the tags it was opened with', async () => {
    overrides.set('/newsletter/audience', (search) => ({ recipients: search?.tags === '7' ? 1 : 99 }));
    renderPage(<ComposeForm defaultAudience="newsletter" defaultTagIds={[7]} onSent={() => {}} />);
    await screen.findByText('1 feliratkozó kapja meg BCC-ben.');
    expect(await screen.findByText('Pékségek')).toBeTruthy();
  });

  it('hands the server recipient count to onSent instead of just the email id', async () => {
    overrides.set('/newsletter/audience', () => ({ recipients: 3 }));
    overrides.set('/emails/preview', () => ({
      subject: 'Akció',
      body_html: '<p>Akció</p>',
      unresolved: [],
      recipient_suppressed: false,
    }));
    overrides.set('/newsletter/sends', () => ({ id: 99, recipients: 3 }));
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
