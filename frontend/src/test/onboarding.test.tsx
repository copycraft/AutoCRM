// Onboarding by invitation (0066) and the rights-based screens next to it: the welcome page
// sets the look and the password, a dead link says why, the HR page offers only the parts
// one holds, and the menu asks for rights, not role names.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, screen, waitFor } from '@testing-library/react';
import { renderPage } from './harness';
import * as f from './fixtures';
import { overrides, resetOverrides, resetWrites, writes } from './client-mock';
import { WelcomeForm } from '@/components/account/WelcomeForm';
import { HrWorkspace } from '@/components/hr/HrWorkspace';
import { Sidebar } from '@/components/layout/Sidebar';
import { ApiError } from '@/lib/api/errors';

vi.mock('@/lib/api/client', () => import('./client-mock'));
vi.mock('next/navigation', () => ({
  usePathname: () => '/hu',
  useRouter: () => ({ push: () => undefined, replace: () => undefined }),
  useSearchParams: () => new URLSearchParams(),
}));

afterEach(() => {
  resetOverrides();
  resetWrites();
  delete document.documentElement.dataset.theme;
});

const asUser = (user: object) => overrides.set('/auth/me', () => ({ user }));
const invite = { email: 'uj@autotherm.hu', display_name: 'Új Kolléga', expires_at: '2026-11-13T10:00:00Z' };

describe('Welcome page', () => {
  it('applies the picked look at once and sends it with the password', async () => {
    overrides.set('/onboarding/tok123', () => invite);
    renderPage(<WelcomeForm token="tok123" />);
    expect(await screen.findByText('Üdvözöljük, Új Kolléga!')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('radio', { name: /Sötét/ }));
    expect(document.documentElement.dataset.theme).toBe('dark');

    fireEvent.change(screen.getByLabelText('Jelszó'), { target: { value: 'hosszu-jelszo-2026' } });
    fireEvent.change(screen.getByLabelText('Jelszó még egyszer'), { target: { value: 'hosszu-jelszo-2026' } });
    fireEvent.click(screen.getByRole('button', { name: 'Fiók beállítása és belépés' }));
    await waitFor(() =>
      expect(writes).toContainEqual({
        path: '/onboarding/tok123',
        method: 'POST',
        body: { password: 'hosszu-jelszo-2026', theme: 'dark' },
      }),
    );
  });

  it('sends nothing while the password is short or the two differ', async () => {
    overrides.set('/onboarding/tok123', () => invite);
    renderPage(<WelcomeForm token="tok123" />);
    fireEvent.change(await screen.findByLabelText('Jelszó'), { target: { value: 'rovid' } });
    fireEvent.change(screen.getByLabelText('Jelszó még egyszer'), { target: { value: 'masik' } });
    fireEvent.click(screen.getByRole('button', { name: 'Fiók beállítása és belépés' }));
    expect(await screen.findByText('Legalább 12 karakter kell.')).toBeInTheDocument();
    expect(screen.getByText('A két jelszó nem egyezik.')).toBeInTheDocument();
    expect(writes).toEqual([]);
  });

  it('says an expired link is expired, and offers the login', async () => {
    overrides.set('/onboarding/old', () => {
      throw new ApiError('invite_expired', 422, 'expired');
    });
    renderPage(<WelcomeForm token="old" />);
    expect(await screen.findByText(/Ez a meghívó lejárt/)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Tovább a bejelentkezéshez' })).toBeInTheDocument();
  });
});

describe('HR by parts', () => {
  it('opens straight on leave for someone who keeps leave only', async () => {
    asUser({ ...f.sessionUser, role: 'designer', hr_access: false, capabilities: ['manage_leave'] });
    renderPage(<HrWorkspace />);
    expect(await screen.findByRole('table', { name: 'Csapatnaptár' })).toBeInTheDocument();
    // One part: no tabs to choose from, and not the directory.
    expect(screen.queryByRole('tablist')).not.toBeInTheDocument();
  });

  it('refuses someone with none of HR', async () => {
    asUser({ ...f.sessionUser, role: 'worker', hr_access: false, capabilities: ['comment'] });
    renderPage(<HrWorkspace />);
    expect(await screen.findByRole('alert')).toBeInTheDocument();
  });
});

describe('Menu by rights', () => {
  it('shows Settings to a configuration grant and HR to a recruitment grant, not Administration', async () => {
    asUser({
      ...f.sessionUser,
      role: 'office',
      hr_access: false,
      capabilities: ['edit_orders', 'manage_configuration', 'manage_recruitment'],
    });
    renderPage(<Sidebar />);
    // These live in the collapsed "Rendszer" section.
    fireEvent.click(await screen.findByRole('button', { name: /Rendszer/ }));
    expect(await screen.findByRole('link', { name: /Beállítások/ })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /^HR$/ })).toBeInTheDocument();
    expect(screen.queryByRole('link', { name: /Adminisztráció/ })).not.toBeInTheDocument();
  });
});
