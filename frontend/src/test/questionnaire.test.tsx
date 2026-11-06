// The build questionnaire: sections open on yes/no answers (heating, cooling, and the freezer
// behind cooling), type-specific sections follow the project type, and a lead converts only
// once every required question asked is answered — the convert dialog puts those first.
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderPage } from './harness';
import { resetWrites, writes } from './client-mock';

vi.mock('@/lib/api/client', () => import('./client-mock'));
vi.mock('next-intl/server', () => import('./intl-server-mock'));

import NewOrderPage from '@/app/[locale]/orders/new/page';
import { LeadConvertDialog } from '@/components/forms/LeadConvertDialog';
import { missingQuestions, visibleSections, answerProblem, formatAnswer } from '@/lib/questionnaire';
import * as f from './fixtures';

const BOX = f.boxProjectType.id;
const VAN = f.vanProjectType.id;
const keys = (xs: { key: string }[]) => xs.map((x) => x.key);

describe('the questionnaire rules (mirroring the server)', () => {
  it('opens a section per switch, both at once, and the freezer only behind cooling', () => {
    const qn = f.questionnaire;
    expect(keys(visibleSections(qn, BOX, {}))).toEqual(['general', 'box']);
    expect(keys(visibleSections(qn, BOX, { heats: true, cools: true }))).toEqual([
      'general',
      'box',
      'cooling',
      'heating',
    ]);
    expect(keys(visibleSections(qn, VAN, { cools: true, freezes: true }))).toEqual([
      'general',
      'cooling',
      'freezer',
    ]);
    expect(keys(visibleSections(qn, VAN, { cools: false, freezes: true }))).toEqual(['general']);
    expect(keys(visibleSections(qn, null, {}))).toEqual(['general']);
  });

  it('counts false as an answer and blank as missing', () => {
    const qn = f.questionnaire;
    expect(keys(missingQuestions(qn, BOX, {}))).toEqual(['heats', 'cools', 'box_length_mm']);
    expect(keys(missingQuestions(qn, BOX, { heats: false, cools: false, box_length_mm: ' ' }))).toEqual([
      'box_length_mm',
    ]);
    expect(
      keys(missingQuestions(qn, BOX, { heats: false, cools: true, box_length_mm: 4200, freezes: true })),
    ).toEqual(['cool_temp_c', 'freeze_temp_c']);
  });

  it('checks a number against its range, accepting a decimal comma', () => {
    const temp = f.questionnaire.sections[2]!.questions[0]!;
    expect(answerProblem(temp, '-18,5')).toBeNull();
    expect(answerProblem(temp, '-50')).toBe('range:-40:30');
    expect(answerProblem(temp, 'hideg')).toBe('numeric');
    expect(formatAnswer(temp, -18.5, 'Igen', 'Nem')).toBe('-18,5 °C');
  });
});

describe('the order form', () => {
  it('asks the box questions for a box, and opens heating and cooling on their answers', async () => {
    const user = userEvent.setup();
    renderPage(<NewOrderPage />);

    const select = (await screen.findByLabelText(/^Projekttípus/i)) as HTMLSelectElement;
    await waitFor(() => expect(select.options.length).toBeGreaterThan(1));
    expect(screen.queryByText('Belső hossz')).toBeNull();
    await user.selectOptions(select, String(BOX));
    expect(await screen.findByText('Belső hossz')).toBeInTheDocument();

    expect(screen.queryByText('Hűtött hőmérséklet')).toBeNull();
    const cools = screen.getByRole('radiogroup', { name: /Hűteni kell/ });
    await user.click(within(cools).getByRole('radio', { name: 'Igen' }));
    expect(await screen.findByText('Hűtött hőmérséklet')).toBeInTheDocument();

    const heats = screen.getByRole('radiogroup', { name: /Fűteni kell/ });
    await user.click(within(heats).getByRole('radio', { name: 'Igen' }));
    expect(await screen.findByText('Fűtött hőmérséklet')).toBeInTheDocument();
    // Both at once: heating did not replace cooling.
    expect(screen.getByText('Hűtött hőmérséklet')).toBeInTheDocument();

    const freezes = screen.getByRole('radiogroup', { name: /Fagyasztani is kell/ });
    await user.click(within(freezes).getByRole('radio', { name: 'Igen' }));
    expect(await screen.findByText('Fagyasztott hőmérséklet')).toBeInTheDocument();

    // Cooling off closes the freezer behind it.
    await user.click(within(cools).getByRole('radio', { name: 'Nem' }));
    await waitFor(() => expect(screen.queryByText('Fagyasztott hőmérséklet')).toBeNull());
  });
});

describe('converting a lead', () => {
  beforeEach(() => resetWrites());

  it('puts the missing answers first and converts only once they are in', async () => {
    const user = userEvent.setup();
    // The lead: a box that cools, its length given; heating, freezing and the temperature
    // are still open.
    renderPage(<LeadConvertDialog leadId={5} detail={f.leadDetail} onClose={() => {}} />);

    const missing = await screen.findByTestId('convert-missing');
    await waitFor(() =>
      expect(within(missing).getByRole('radiogroup', { name: /Fűteni kell/ })).toBeInTheDocument(),
    );
    expect(within(missing).getByText('Hűtött hőmérséklet')).toBeInTheDocument();
    expect(within(missing).getByRole('radiogroup', { name: /Fagyasztani is kell/ })).toBeInTheDocument();
    // Already answered on the lead: not asked again at the top.
    expect(within(missing).queryByText('Belső hossz')).toBeNull();

    const convert = screen.getByRole('button', { name: 'Átalakítás megrendeléssé' });
    expect(convert).toBeDisabled();

    await user.click(within(within(missing).getByRole('radiogroup', { name: /Fűteni kell/ })).getByRole('radio', { name: 'Nem' }));
    await user.type(within(missing).getByLabelText(/Hűtött hőmérséklet/), '-18,5');
    // Freezing opens its own required question, and it joins the top block.
    await user.click(
      within(within(missing).getByRole('radiogroup', { name: /Fagyasztani is kell/ })).getByRole('radio', { name: 'Igen' }),
    );
    const freezeTemp = await within(missing).findByLabelText(/Fagyasztott hőmérséklet/);
    expect(convert).toBeDisabled();
    await user.type(freezeTemp, '-22');

    // Answered questions stay where they were typed.
    expect(within(missing).getByLabelText(/Hűtött hőmérséklet/)).toHaveValue('-18,5');
    await waitFor(() => expect(convert).toBeEnabled());
    await user.click(convert);

    await waitFor(() => expect(writes.some((w) => w.path === '/leads/5/convert')).toBe(true));
    const sent = writes.find((w) => w.path === '/leads/5/convert')!.body as Record<string, unknown>;
    expect(sent.project_type_id).toBe(BOX);
    expect(sent.spec_answers).toEqual({
      cools: true,
      box_length_mm: 4200,
      heats: false,
      cool_temp_c: '-18,5',
      freezes: true,
      freeze_temp_c: '-22',
    });
  });
});
