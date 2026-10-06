// HR recruitment: the listings tab, and the public application form a listing's link opens.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, screen, waitFor } from '@testing-library/react';
import { renderPage } from './harness';
import * as f from './fixtures';
import { overrides, resetOverrides, resetWrites, writes } from './client-mock';
import { RecruitmentSection } from '@/components/hr/RecruitmentSection';
import { JobApplyForm } from '@/components/recruitment/JobApplyForm';

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

describe('Recruitment tab', () => {
  it('lists each listing with its status and how many applied', async () => {
    renderPage(<RecruitmentSection />);
    expect(await screen.findByText('Hűtős szerelő')).toBeInTheDocument();
    expect(screen.getByText('Piszkozat')).toBeInTheDocument();
    expect(screen.getByText('1 jelentkező')).toBeInTheDocument();
  });

  it('creates a listing from the dialog', async () => {
    renderPage(<RecruitmentSection />);
    fireEvent.click(await screen.findByRole('button', { name: 'Új álláshirdetés' }));
    fireEvent.change(await screen.findByLabelText(/^Pozíció megnevezése/), { target: { value: ' Lakatos ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Mentés' }));
    await waitFor(() => expect(writes).toHaveLength(1));
    expect(writes[0]).toMatchObject({
      path: '/hr/jobs',
      method: 'POST',
      body: { title: ' Lakatos ', location: null, description: null },
    });
  });

  it('shows the link to share and the applicant with a resume download', async () => {
    renderPage(<RecruitmentSection />);
    fireEvent.click(await screen.findByRole('button', { name: 'Jelentkezők' }));
    expect(await screen.findByDisplayValue(f.jobPosting.public_url)).toBeInTheDocument();
    // A draft says its link does not work yet, and offers to publish it.
    expect(screen.getByText(/a link még nem működik/)).toBeInTheDocument();
    expect(await screen.findByText('Tóth Gábor')).toBeInTheDocument();
    expect(screen.getByText('31 éves')).toBeInTheDocument();
    const resume = screen.getByRole('link', { name: /Önéletrajz letöltése/ });
    expect(resume).toHaveAttribute('href', 'https://files.example/resume.pdf');

    fireEvent.click(screen.getByRole('button', { name: 'Meghirdetés' }));
    await waitFor(() => expect(writes).toHaveLength(1));
    expect(writes[0]).toMatchObject({ path: '/hr/jobs/3/publish', method: 'POST' });
  });

  it('deletes an applicant profile only after confirming', async () => {
    renderPage(<RecruitmentSection />);
    fireEvent.click(await screen.findByRole('button', { name: 'Jelentkezők' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Profil törlése' }));
    expect(writes).toHaveLength(0);
    fireEvent.click(await screen.findByRole('button', { name: 'Megerősítés' }));
    await waitFor(() => expect(writes).toHaveLength(1));
    expect(writes[0]).toMatchObject({ path: '/hr/applications/11', method: 'DELETE' });
  });

  it('saves notes from the phone interview', async () => {
    renderPage(<RecruitmentSection />);
    fireEvent.click(await screen.findByRole('button', { name: 'Jelentkezők' }));
    const notes = await screen.findByLabelText('Jegyzetek');
    fireEvent.change(notes, { target: { value: 'Jó benyomás telefonon.' } });
    fireEvent.click(screen.getByRole('button', { name: 'Mentés' }));
    await waitFor(() => expect(writes).toHaveLength(1));
    expect(writes[0]).toMatchObject({
      path: '/hr/applications/11',
      method: 'PATCH',
      body: { notes: 'Jó benyomás telefonon.' },
    });
  });
});

describe('Public application form', () => {
  const open = { title: 'Hűtős szerelő', description: 'Műszakban.', location: 'Budapest', status: 'published' };

  it('shows the listing and refuses to send an incomplete form', async () => {
    overrides.set('/public/jobs/abc', () => open);
    renderPage(<JobApplyForm slug="abc" />);
    expect(await screen.findByRole('heading', { name: 'Hűtős szerelő' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Jelentkezés elküldése' }));
    expect((await screen.findAllByText('Ez a mező kötelező.')).length).toBeGreaterThanOrEqual(4);
    expect(writes).toHaveLength(0);
  });

  it('rejects a resume of the wrong type before sending anything', async () => {
    overrides.set('/public/jobs/abc', () => open);
    renderPage(<JobApplyForm slug="abc" />);
    const input = await screen.findByLabelText(/^Önéletrajz/);
    fireEvent.change(input, { target: { files: [new File(['x'], 'virus.exe')] } });
    expect(await screen.findByText(/Csak PDF, DOC, DOCX, ODT vagy RTF/)).toBeInTheDocument();
  });

  it('sends the details and the resume, then thanks the applicant', async () => {
    overrides.set('/public/jobs/abc', () => open);
    renderPage(<JobApplyForm slug="abc" />);
    fireEvent.change(await screen.findByLabelText(/^Teljes név/), { target: { value: ' Tóth Gábor ' } });
    fireEvent.change(screen.getByLabelText(/^E-mail/), { target: { value: 'gabor@example.hu' } });
    fireEvent.change(screen.getByLabelText(/^Telefonszám/), { target: { value: '+36 30 999 8888' } });
    fireEvent.change(screen.getByLabelText(/^Életkor/), { target: { value: '31' } });
    fireEvent.change(screen.getByLabelText(/^Önéletrajz/), {
      target: { files: [new File(['%PDF-1.4'], 'cv.pdf', { type: 'application/pdf' })] },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Jelentkezés elküldése' }));
    expect(await screen.findByText('Köszönjük a jelentkezését!')).toBeInTheDocument();
    expect(writes).toHaveLength(1);
    expect(writes[0]).toMatchObject({
      path: '/public/jobs/abc/applications',
      method: 'POST',
      body: {
        full_name: 'Tóth Gábor',
        email: 'gabor@example.hu',
        phone: '+36 30 999 8888',
        age: '31',
        resume: 'cv.pdf',
        // The honeypot goes along empty: a person never fills it.
        company: '',
      },
    });
  });

  it('says so when the position is closed, with no form', async () => {
    overrides.set('/public/jobs/abc', () => ({ ...open, status: 'closed' }));
    renderPage(<JobApplyForm slug="abc" />);
    expect(await screen.findByText('Ez a pozíció már nem elérhető')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Jelentkezés elküldése' })).not.toBeInTheDocument();
  });
});
