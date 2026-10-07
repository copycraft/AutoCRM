// One smoke test per route (V0.2).
//
// These exist because `tsc --noEmit`, `next lint` and `next build` all passed on
// an order detail screen that printed `editing ? (` to the user. The assertion
// that matters is expectNoSourceText: nothing that looks like JSX source may
// reach the DOM as text.
import { describe, expect, it, vi } from 'vitest';
import { screen, waitFor } from '@testing-library/react';
import type { ReactElement } from 'react';
import { expectNoSourceText, renderPage } from './harness';

vi.mock('@/lib/api/client', () => import('./client-mock'));
vi.mock('next-intl/server', () => import('./intl-server-mock'));

import AdminPage from '@/app/[locale]/admin/page';
import BlockersPage from '@/app/[locale]/blockers/page';
import BoardPage from '@/app/[locale]/board/page';
import DashboardPage from '@/app/[locale]/page';
import EmailDetailPage from '@/app/[locale]/emails/[id]/page';
import NewEmailPage from '@/app/[locale]/emails/new/page';
import UnsubscribePage from '@/app/[locale]/newsletter/unsubscribe/page';
import ConfirmPage from '@/app/[locale]/newsletter/confirm/page';
import EmailsPage from '@/app/[locale]/emails/page';
import LeadDetailPage from '@/app/[locale]/leads/[id]/page';
import LeadNewPage from '@/app/[locale]/leads/new/page';
import LeadsPage from '@/app/[locale]/leads/page';
import LoginPage from '@/app/[locale]/login/page';
import OrderDetailPage from '@/app/[locale]/orders/[id]/page';
import OrderNewPage from '@/app/[locale]/orders/new/page';
import OrdersPage from '@/app/[locale]/orders/page';
import PartnerDetailPage from '@/app/[locale]/partners/[id]/page';
import PartnerNewPage from '@/app/[locale]/partners/new/page';
import PartnersBusinessPage from '@/app/[locale]/partners/business/page';
import PartnersConsumersPage from '@/app/[locale]/partners/consumers/page';
import PasswordPage from '@/app/[locale]/password/page';
import PreferencesPage from '@/app/[locale]/preferences/page';
import ReportsPage from '@/app/[locale]/reports/page';
import SettingsPage from '@/app/[locale]/settings/page';
import MarketingPage from '@/app/[locale]/marketing/page';
import IncomingInvoicesPage from '@/app/[locale]/incoming-invoices/page';
import TemplatesPage from '@/app/[locale]/templates/page';

const locale = { locale: 'hu' };

/** A route: how to build its element, and a string only a loaded page shows. */
interface Route {
  path: string;
  element: () => ReactElement | Promise<ReactElement>;
  expect: string | RegExp;
}

const routes: Route[] = [
  { path: '/hu', element: () => <DashboardPage />, expect: 'Irányítópult' },
  { path: '/hu/login', element: () => <LoginPage />, expect: /Bejelentkez/ },
  { path: '/hu/password', element: () => <PasswordPage />, expect: /jelszó/i },
  { path: '/hu/preferences', element: () => <PreferencesPage />, expect: /Saját beállítások/ },
  { path: '/hu/partners/business', element: () => <PartnersBusinessPage />, expect: /Müller/ },
  { path: '/hu/partners/consumers', element: () => <PartnersConsumersPage />, expect: /Müller/ },
  { path: '/hu/partners/new', element: () => <PartnerNewPage />, expect: /Új partner/i },
  {
    path: '/hu/partners/[id]',
    element: () => <PartnerDetailPage params={{ id: '7' }} />,
    expect: /Müller/,
  },
  { path: '/hu/leads', element: () => <LeadsPage />, expect: /Három Sprinter/ },
  { path: '/hu/leads/new', element: () => <LeadNewPage />, expect: /Új lead/i },
  {
    path: '/hu/leads/[id]',
    element: () => <LeadDetailPage params={{ id: '5' }} />,
    expect: /Három Sprinter/,
  },
  { path: '/hu/orders', element: () => <OrdersPage />, expect: /MC-1001/ },
  { path: '/hu/orders/new', element: () => <OrderNewPage />, expect: /Új megrendelés/i },
  {
    path: '/hu/orders/[id]',
    element: () => <OrderDetailPage params={{ id: '3' }} />,
    expect: /MC-1001/,
  },
  { path: '/hu/blockers', element: () => BlockersPage({ params: Promise.resolve(locale) }), expect: /Akadályok/ },
  { path: '/hu/board', element: () => <BoardPage />, expect: /Átvehető/ },
  { path: '/hu/emails', element: () => <EmailsPage />, expect: /fenyezo\.hu/ },
  {
    path: '/hu/emails/[id]',
    element: () => <EmailDetailPage params={{ id: '61' }} />,
    expect: /fenyezo\.hu/,
  },
  {
    path: '/hu/emails/new',
    element: () => <NewEmailPage />,
    expect: /Markdown/,
  },
  {
    path: '/hu/newsletter/unsubscribe',
    element: () => <UnsubscribePage searchParams={Promise.resolve({})} />,
    expect: /Leiratkozás/,
  },
  {
    path: '/hu/newsletter/confirm',
    element: () => <ConfirmPage searchParams={Promise.resolve({})} />,
    expect: /megerősítése/,
  },
  { path: '/hu/reports', element: () => ReportsPage({ params: Promise.resolve(locale) }), expect: /Jelentések/ },
  { path: '/hu/settings', element: () => <SettingsPage />, expect: /smtp\.example\.com|Beállítások/ },
  { path: '/hu/marketing', element: () => <MarketingPage />, expect: /info@pekseg.hu/ },
  { path: '/hu/incoming-invoices', element: () => <IncomingInvoicesPage />, expect: /Hűtőgép Kft/ },
  { path: '/hu/templates', element: () => <TemplatesPage />, expect: /Árajánlat utánkövetés/ },
  { path: '/hu/admin', element: () => AdminPage({ params: Promise.resolve(locale) }), expect: /Adminisztráció/i },
];

describe.each(routes)('$path', (route) => {
  it('renders without leaking JSX source into the page', async () => {
    const element = await route.element();
    const { container } = renderPage(element);
    await waitFor(() => {
      expect(screen.getAllByText(route.expect).length).toBeGreaterThan(0);
    });
    expectNoSourceText(container);
  });
});

describe('the order detail tabs', () => {
  it('shows only the read view of the data tab, not the edit form as well', async () => {
    renderPage(<OrderDetailPage params={{ id: '3' }} />);
    await waitFor(() => expect(screen.getAllByText(/MC-1001/).length).toBeGreaterThan(0));
    // The unbraced ternary rendered both branches: the read-only grid and the
    // edit form appeared at once, with the source text between them.
    expect(screen.queryByText('editing ? (')).toBeNull();
    expect(screen.queryByRole('button', { name: 'Mentés' })).toBeNull();
  });
});
