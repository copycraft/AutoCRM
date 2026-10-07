// The HR module and the users page an admin grants access from.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, screen, waitFor } from '@testing-library/react';
import { renderPage } from './harness';
import * as f from './fixtures';
import { overrides, resetOverrides, resetWrites, writes } from './client-mock';
import { EmployeeDirectory } from '@/components/hr/EmployeeDirectory';
import { UsersAdmin } from '@/components/admin/UsersAdmin';
import { Sidebar } from '@/components/layout/Sidebar';
import { LeaveSection } from '@/components/hr/LeaveSection';
import { NotificationList } from '@/components/notifications/NotificationList';

vi.mock('@/lib/api/client', () => import('./client-mock'));
vi.mock('next/navigation', () => ({
  usePathname: () => '/hu',
  useRouter: () => ({ push: () => undefined, replace: () => undefined }),
  useSearchParams: () => new URLSearchParams(),
}));

afterEach(() => {
  resetOverrides();
  resetWrites();
});

const asUser = (user: object) => overrides.set('/auth/me', () => ({ user }));

describe('HR directory', () => {
  it('shows each employee with both numbers and the email', async () => {
    renderPage(<EmployeeDirectory />);
    expect(await screen.findByText('Kiss Péter')).toBeInTheDocument();
    expect(screen.getByText('+36 30 111 2222')).toBeInTheDocument();
    expect(screen.getByText('+36 20 333 4444')).toBeInTheDocument();
    expect(screen.getByText('peter@autotherm.hu')).toBeInTheDocument();
  });

  it('does not open for a user without HR access', async () => {
    asUser({ ...f.sessionUser, role: 'office', hr_access: false });
    renderPage(<EmployeeDirectory />);
    expect(await screen.findByRole('alert')).toBeInTheDocument();
    expect(screen.queryByText('Kiss Péter')).not.toBeInTheDocument();
  });

  it('opens for an office user an admin granted access', async () => {
    asUser({ ...f.sessionUser, role: 'office', hr_access: true });
    renderPage(<EmployeeDirectory />);
    expect(await screen.findByText('Kiss Péter')).toBeInTheDocument();
  });

  it('creates an employee from the dialog, sending blanks as null', async () => {
    renderPage(<EmployeeDirectory />);
    fireEvent.click(await screen.findByRole('button', { name: 'Új munkatárs' }));
    fireEvent.change(await screen.findByLabelText(/^Név/), { target: { value: ' Nagy Anna ' } });
    fireEvent.change(screen.getByLabelText('Céges telefonszám'), { target: { value: '+36 1 555 0000' } });
    fireEvent.click(screen.getByRole('button', { name: 'Mentés' }));
    await waitFor(() => expect(writes).toHaveLength(1));
    expect(writes[0]).toMatchObject({
      path: '/hr/employees',
      method: 'POST',
      body: {
        full_name: ' Nagy Anna ',
        email: null,
        company_phone: '+36 1 555 0000',
        personal_phone: null,
      },
    });
  });

  it('cannot save without a name', async () => {
    renderPage(<EmployeeDirectory />);
    fireEvent.click(await screen.findByRole('button', { name: 'Új munkatárs' }));
    expect(await screen.findByRole('button', { name: 'Mentés' })).toBeDisabled();
  });
});

describe('Sidebar HR entry', () => {
  it('is hidden without HR access and shown with it', async () => {
    asUser({ ...f.sessionUser, role: 'office', hr_access: false });
    const { unmount } = renderPage(<Sidebar />);
    await screen.findByText('Megrendelések');
    expect(screen.queryByRole('link', { name: 'HR' })).not.toBeInTheDocument();
    unmount();

    asUser({ ...f.sessionUser, role: 'office', hr_access: true });
    renderPage(<Sidebar />);
    // HR lives in the collapsed "Rendszer" section.
    fireEvent.click(await screen.findByRole('button', { name: /Rendszer/ }));
    expect(await screen.findByRole('link', { name: 'HR' })).toBeInTheDocument();
  });
});

describe('Users page', () => {
  it('lists every user and lets an admin grant HR access', async () => {
    renderPage(<UsersAdmin />);
    expect(await screen.findByText('Nagy Anna')).toBeInTheDocument();
    expect(screen.getByText('Iroda Ilona')).toBeInTheDocument();
    // Admins always have it: no checkbox for them, one for everyone else.
    expect(screen.getByText('Mindig')).toBeInTheDocument();
    const box = screen.getByLabelText('HR-hozzáférés: Nagy Anna');
    expect(box).not.toBeChecked();
    fireEvent.click(box);
    await waitFor(() => expect(writes).toHaveLength(1));
    expect(writes[0]).toMatchObject({ path: '/users/2', method: 'PATCH', body: { hr_access: true } });
  });

  it('does not let an admin demote or deactivate themselves', async () => {
    renderPage(<UsersAdmin />);
    await screen.findByText('Nagy Anna');
    expect(screen.getByLabelText('Szerepkör: Iroda Ilona')).toBeDisabled();
    expect(screen.getByLabelText('Aktív: Iroda Ilona')).toBeDisabled();
    expect(screen.getByLabelText('Szerepkör: Nagy Anna')).toBeEnabled();
  });

  it('is closed to everyone but admins', async () => {
    asUser({ ...f.sessionUser, role: 'office', hr_access: true });
    renderPage(<UsersAdmin />);
    expect(await screen.findByRole('alert')).toBeInTheDocument();
    expect(screen.queryByText('Nagy Anna')).not.toBeInTheDocument();
  });
});

describe('Leave', () => {
  it('shows the month calendar, the month absences and the balances', async () => {
    renderPage(<LeaveSection />);
    expect(await screen.findByRole('table', { name: 'Csapatnaptár' })).toBeInTheDocument();
    // The absence is listed with its kind, dates, working days and note.
    expect(await screen.findByText('Nyaralás')).toBeInTheDocument();
    expect(screen.getByText('3 munkanap')).toBeInTheDocument();
    // The balance: over the allowance reads as a negative number.
    expect(screen.getByText('-3')).toBeInTheDocument();
    expect(screen.getByText('Szabadságegyenleg', { exact: false })).toBeInTheDocument();
  });

  it('books an absence with the dates and kind chosen', async () => {
    renderPage(<LeaveSection />);
    fireEvent.click(await screen.findByRole('button', { name: 'Új távollét' }));
    fireEvent.change(await screen.findByLabelText('Típus'), { target: { value: 'sick' } });
    fireEvent.change(screen.getByLabelText('Kezdete'), { target: { value: '2026-06-08' } });
    fireEvent.change(screen.getByLabelText('Utolsó nap'), { target: { value: '2026-06-10' } });
    fireEvent.click(screen.getByRole('button', { name: 'Mentés' }));
    await waitFor(() => expect(writes).toHaveLength(1));
    expect(writes[0]).toMatchObject({
      path: '/hr/employees/7/absences',
      method: 'POST',
      body: { kind: 'sick', start_date: '2026-06-08', end_date: '2026-06-10', note: null },
    });
  });

  it('removes an absence after confirmation', async () => {
    renderPage(<LeaveSection />);
    fireEvent.click(await screen.findByRole('button', { name: 'Távollét törlése: Kiss Péter' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Megerősítés' }));
    await waitFor(() => expect(writes).toHaveLength(1));
    expect(writes[0]).toMatchObject({ path: '/hr/absences/31', method: 'DELETE' });
  });
});

describe('Notifications', () => {
  it('shows an unread badge in the menu', async () => {
    renderPage(<Sidebar />);
    expect(await screen.findByLabelText('1 olvasatlan')).toBeInTheDocument();
  });

  it('lists them, marks one read when opened, and can mark all read', async () => {
    renderPage(<NotificationList />);
    expect(await screen.findByText('Új érdeklődés a weboldalról')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Új érdeklődés a weboldalról'));
    await waitFor(() => expect(writes).toHaveLength(1));
    expect(writes[0]).toMatchObject({ path: '/notifications/read', method: 'POST', body: { ids: [12] } });
    fireEvent.click(screen.getByRole('button', { name: 'Mind olvasott' }));
    await waitFor(() => expect(writes).toHaveLength(2));
    expect(writes[1]).toMatchObject({ path: '/notifications/read-all', method: 'POST' });
  });
});
