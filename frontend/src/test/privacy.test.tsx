// The public data-protection page: it always explains the basics, and shows the company's
// details only where they were configured, never a placeholder.
import { describe, expect, it } from 'vitest';
import { screen, within } from '@testing-library/react';
import { renderPage } from './harness';
import { PrivacyContent } from '@/components/privacy/PrivacyContent';

describe('privacy page', () => {
  it('explains who has the data, what is kept and what the reader can do', () => {
    renderPage(<PrivacyContent company={{}} />);
    expect(screen.getByRole('heading', { level: 1, name: 'Adatkezelés az Autotherm-nél' })).toBeInTheDocument();
    for (const heading of [
      'Hogyan kerültél ide?',
      'Milyen rendszerben tároljuk az adatokat?',
      'Kik vagyunk?',
      'Milyen adatokat kezelünk, és miért?',
      'Kinek adjuk át az adataidat?',
      'Mit tehetsz?',
      'Ha úgy érzed, hogy jogsértés történt',
      'Kapcsolat',
    ]) {
      expect(screen.getByRole('heading', { name: heading })).toBeInTheDocument();
    }
    // The table covers every kind of person whose data the system holds.
    const table = screen.getByRole('table');
    for (const who of ['Ajánlatot kérők, érdeklődők', 'Ügyfelek, partnerek és kapcsolattartóik', 'Számlázás', 'Hírlevélre feliratkozók']) {
      expect(within(table).getByText(who)).toBeInTheDocument();
    }
    // Staff are told separately.
    expect(screen.getByText('A munkatársaink adatairól külön tájékoztatást adunk.')).toBeInTheDocument();
  });

  it('links to the newsletter unsubscribe page', () => {
    renderPage(<PrivacyContent company={{}} />);
    expect(screen.getByRole('link', { name: 'Leiratkozás a hírlevélről' })).toHaveAttribute(
      'href',
      '/hu/newsletter/unsubscribe',
    );
  });

  it('shows only the company details that are configured, and no placeholders', () => {
    renderPage(<PrivacyContent company={{ name: 'Autotherm Kft.', email: 'adatvedelem@autotherm.hu' }} />);
    const contact = screen.getByRole('region', { name: 'Kapcsolat' });
    expect(within(contact).getByText('Autotherm Kft.')).toBeInTheDocument();
    expect(within(contact).getByRole('link', { name: 'adatvedelem@autotherm.hu' })).toHaveAttribute(
      'href',
      'mailto:adatvedelem@autotherm.hu',
    );
    // Not configured: no label, no empty value, no "[cím]".
    for (const label of ['Székhely', 'Cégjegyzékszám / adószám', 'Telefon']) {
      expect(within(contact).queryByText(label)).not.toBeInTheDocument();
    }
    expect(document.body.textContent).not.toMatch(/\[[^\]]+\]/);
    // And no button to a full notice that has no address yet.
    expect(screen.queryByRole('link', { name: 'A teljes Adatkezelési tájékoztató' })).not.toBeInTheDocument();
  });

  it('falls back to the brand name for the controller, and links the full notice when there is one', () => {
    const { unmount } = renderPage(<PrivacyContent company={{}} />);
    expect(within(screen.getByRole('region', { name: 'Kapcsolat' })).getByText('Autotherm')).toBeInTheDocument();
    unmount();

    renderPage(
      <PrivacyContent
        company={{ phone: '+36 30 123 4567', noticeUrl: 'https://autotherm.hu/adatkezelesi-tajekoztato' }}
      />,
    );
    expect(screen.getByRole('link', { name: '+36 30 123 4567' })).toHaveAttribute('href', 'tel:+36301234567');
    expect(screen.getByRole('link', { name: 'A teljes Adatkezelési tájékoztató' })).toHaveAttribute(
      'href',
      'https://autotherm.hu/adatkezelesi-tajekoztato',
    );
  });
});
