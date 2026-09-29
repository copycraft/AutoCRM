import { render, waitFor } from '@testing-library/react';
import { useQueryClient, type QueryClient } from '@tanstack/react-query';
import { describe, expect, it } from 'vitest';
import { QueryProvider, qk } from '@/lib/query/provider';
import { ApiError } from '@/lib/api/errors';

// docs/history/FRONTEND_PLAN.md §14: "401 returns to login". The AppShell redirects when the cached
// `me` is empty, so any 401 — not only one from /auth/me — has to empty it.

function grabClient(): QueryClient {
  let client: QueryClient | null = null;
  function Grab() {
    client = useQueryClient();
    return null;
  }
  render(
    <QueryProvider>
      <Grab />
    </QueryProvider>,
  );
  if (!client) throw new Error('no client');
  return client;
}

const signedIn = {
  user: {
    id: 1,
    email: 'a@autotherm.test',
    display_name: 'A',
    role: 'office',
    must_change_password: false,
    session_kind: 'web',
  },
};

const expired = () => new ApiError('unauthenticated', 401, 'authentication required');

describe('session expiry on the web', () => {
  it('a 401 from any query signs the user out so the shell returns to login', async () => {
    const qc = grabClient();
    qc.setQueryData(qk.me, signedIn);
    await qc
      .fetchQuery({ queryKey: ['orders', {}], queryFn: () => Promise.reject(expired()) })
      .catch(() => undefined);
    await waitFor(() => expect(qc.getQueryData(qk.me)).toBeNull());
  });

  it('a 401 from a mutation signs the user out too', async () => {
    const qc = grabClient();
    qc.setQueryData(qk.me, signedIn);
    await qc
      .getMutationCache()
      .build(qc, { mutationFn: () => Promise.reject(expired()) })
      .execute(undefined)
      .catch(() => undefined);
    await waitFor(() => expect(qc.getQueryData(qk.me)).toBeNull());
  });

  it('a 403 does not sign the user out', async () => {
    const qc = grabClient();
    qc.setQueryData(qk.me, signedIn);
    await qc
      .fetchQuery({
        queryKey: ['users'],
        queryFn: () => Promise.reject(new ApiError('forbidden', 403, 'no')),
      })
      .catch(() => undefined);
    expect(qc.getQueryData(qk.me)).toEqual(signedIn);
  });
});
