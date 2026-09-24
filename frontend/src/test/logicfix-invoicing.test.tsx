// Invoicing logic fixes: behaviour the order screen's invoice and proforma sections owe
// the office, each named after the rule it checks.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, screen, waitFor } from '@testing-library/react';
import { renderPage } from './harness';
import * as f from './fixtures';
import { overrides, resetOverrides } from './client-mock';
import { InvoicesSection, invoicePollInterval } from '@/components/orders/InvoicesSection';
import { ProformasSection } from '@/components/orders/ProformasSection';
import type { Invoice } from '@/lib/api/types';

vi.mock('@/lib/api/client', () => import('./client-mock'));

afterEach(() => resetOverrides());

function withInvoices(items: Invoice[]) {
  overrides.set('/orders/3/invoices', () => ({ items }));
}

describe('InvoicesSection', () => {
  it('shows the note of a failed attempt while the queue keeps retrying', async () => {
    withInvoices([
      {
        ...f.submittingInvoice,
        nav_message: 'the NAV sidecar is unreachable: connection refused',
      },
    ]);
    renderPage(<InvoicesSection orderId={3} currency="HUF" />);
    expect(
      await screen.findByText(/the NAV sidecar is unreachable: connection refused/),
    ).toBeInTheDocument();
  });

  it('shows NAV refusing an annulment on an invoice that stays issued', async () => {
    withInvoices([
      {
        ...f.issuedInvoice,
        nav_error_code: 'ANNULMENT_REFERENCE_NOT_FOUND',
        nav_message: 'NAV refused the annulment',
      },
    ]);
    renderPage(<InvoicesSection orderId={3} currency="HUF" />);
    expect(await screen.findByText(/NAV refused the annulment/)).toBeInTheDocument();
    expect(screen.getByText(/ANNULMENT_REFERENCE_NOT_FOUND/)).toBeInTheDocument();
  });

  it('does not offer a storno for an invoice whose storno is already being reported', async () => {
    withInvoices([
      {
        ...f.submittingInvoice,
        id: 510,
        number: 'AT2026-0004',
        kind: 'storno',
        original_invoice_id: f.issuedInvoice.id,
      },
      f.issuedInvoice,
    ]);
    renderPage(<InvoicesSection orderId={3} currency="HUF" />);
    await screen.findByText(f.issuedInvoice.number);
    expect(screen.queryByRole('button', { name: 'Sztornó' })).toBeNull();
  });

  it('offers a storno once the earlier storno attempt was rejected', async () => {
    withInvoices([
      {
        ...f.rejectedInvoice,
        id: 511,
        number: 'AT2026-0005',
        kind: 'storno',
        original_invoice_id: f.issuedInvoice.id,
      },
      f.issuedInvoice,
    ]);
    renderPage(<InvoicesSection orderId={3} currency="HUF" />);
    await screen.findByText(f.issuedInvoice.number);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Sztornó' })).toBeInTheDocument(),
    );
  });

  it('offers to fetch the PDF again for an issued invoice filed without one', async () => {
    withInvoices([{ ...f.issuedInvoice, document_id: null }]);
    const filed = { ...f.issuedInvoice, document_id: 801 };
    let refetched = 0;
    overrides.set('/invoices/501/pdf', () => {
      refetched += 1;
      // The next list poll sees the filed PDF.
      overrides.set('/orders/3/invoices', () => ({ items: [filed] }));
      return filed;
    });
    renderPage(<InvoicesSection orderId={3} currency="HUF" />);
    fireEvent.click(await screen.findByRole('button', { name: 'PDF újratöltése' }));
    await waitFor(() => expect(refetched).toBe(1));
    // The row now links the filed PDF instead of offering the refetch.
    await screen.findByText('PDF');
    expect(screen.queryByRole('button', { name: 'PDF újratöltése' })).toBeNull();
  });

  it('offers no PDF refetch once the invoice already has its PDF', async () => {
    withInvoices([f.issuedInvoice]);
    renderPage(<InvoicesSection orderId={3} currency="HUF" />);
    await screen.findByText('PDF');
    expect(screen.queryByRole('button', { name: 'PDF újratöltése' })).toBeNull();
  });
});

describe('invoicePollInterval', () => {
  const now = Date.parse('2026-09-23T10:00:00Z');

  it('keeps polling while an invoice is being reported', () => {
    expect(invoicePollInterval([f.submittingInvoice], now)).toBe(3000);
  });

  it('keeps polling until the PDF of a just-issued invoice is attached', () => {
    const justIssued = { ...f.issuedInvoice, document_id: null, issued_at: '2026-09-23T09:59:50Z' };
    expect(invoicePollInterval([justIssued], now)).toBe(3000);
  });

  it('stops polling once every invoice is decided and filed', () => {
    expect(invoicePollInterval([f.issuedInvoice, f.rejectedInvoice], now)).toBe(false);
  });

  it('does not poll forever for a PDF that never arrived', () => {
    const old = { ...f.issuedInvoice, document_id: null, issued_at: '2026-09-23T09:00:00Z' };
    expect(invoicePollInterval([old], now)).toBe(false);
  });
});

describe('ProformasSection', () => {
  it('renders the payment due date like every other date', async () => {
    overrides.set('/orders/3/proformas', () => ({ items: [f.proforma] }));
    renderPage(<ProformasSection orderId={3} currency="HUF" />);
    await screen.findByText(f.proforma.number);
    expect(screen.queryByText(/2026-09-29/)).toBeNull();
    expect(screen.getByText(/2026\. 09\. 29\./)).toBeInTheDocument();
  });
});
