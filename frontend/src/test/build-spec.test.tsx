// The conditional build-spec section: which fields appear is decided by the chosen
// project type's `spec_form`, not by a hardcoded list of types.
import { describe, expect, it, vi } from 'vitest';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderPage } from './harness';

vi.mock('@/lib/api/client', () => import('./client-mock'));
vi.mock('next-intl/server', () => import('./intl-server-mock'));

import NewOrderPage from '@/app/[locale]/orders/new/page';
import * as f from './fixtures';

/** The project-type select, once the options have loaded. */
async function projectTypeSelect(): Promise<HTMLSelectElement> {
  return (await screen.findByLabelText(/Projekttípus/i)) as HTMLSelectElement;
}

describe('the build specification section', () => {
  it('shows the cooling fields for a cooling project type', async () => {
    const user = userEvent.setup();
    renderPage(<NewOrderPage />);

    const select = await projectTypeSelect();
    await waitFor(() => expect(select.options.length).toBeGreaterThan(1));
    await user.selectOptions(select, String(f.coolingProjectType.id));

    expect(await screen.findByLabelText(/Hűtőgép gyártó/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/ATP osztály/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/Kívánt hőmérséklet/i)).toBeInTheDocument();
    // The other variant's fields must not be on the page at all — a heater make on a
    // refrigeration job is a lie the database would refuse anyway.
    expect(screen.queryByLabelText(/Fűtőkészülék gyártó/i)).toBeNull();
    expect(screen.queryByLabelText(/Fűtőteljesítmény/i)).toBeNull();
  });

  it('swaps to the heating fields for a heating project type', async () => {
    const user = userEvent.setup();
    renderPage(<NewOrderPage />);

    const select = await projectTypeSelect();
    await waitFor(() => expect(select.options.length).toBeGreaterThan(1));
    await user.selectOptions(select, String(f.coolingProjectType.id));
    expect(await screen.findByLabelText(/Hűtőgép gyártó/i)).toBeInTheDocument();

    await user.selectOptions(select, String(f.heatingProjectType.id));
    expect(await screen.findByLabelText(/Fűtőkészülék gyártó/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/Energiaforrás/i)).toBeInTheDocument();
    // Shared fields survive the swap.
    expect(screen.getByLabelText(/Kívánt hőmérséklet/i)).toBeInTheDocument();
    expect(screen.queryByLabelText(/Hűtőgép gyártó/i)).toBeNull();
    expect(screen.queryByLabelText(/ATP osztály/i)).toBeNull();
  });

  it('shows no specification section for a project type that has none', async () => {
    const user = userEvent.setup();
    renderPage(<NewOrderPage />);

    const select = await projectTypeSelect();
    await waitFor(() => expect(select.options.length).toBeGreaterThan(1));
    await user.selectOptions(select, String(f.plainProjectType.id));

    expect(screen.queryByLabelText(/Kívánt hőmérséklet/i)).toBeNull();
    expect(screen.queryByLabelText(/Hűtőgép gyártó/i)).toBeNull();
    expect(screen.queryByLabelText(/Fűtőkészülék gyártó/i)).toBeNull();
  });

  it('shows nothing before a project type is chosen', async () => {
    renderPage(<NewOrderPage />);
    await projectTypeSelect();
    expect(screen.queryByLabelText(/Kívánt hőmérséklet/i)).toBeNull();
  });
});
