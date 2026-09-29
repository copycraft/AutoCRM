// DateDisplay: timestamps younger than a week read relative, calendar dates never do.
//
// The bug this covers: a payment due date equal to today rendered as "körülbelül 10 órája"
// ("about 10 hours ago"), and the invoicing test that checks due dates failed for exactly
// the week after its fixture date. The clock is pinned here so the result does not depend
// on the day the suite runs.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { DateDisplay } from '@/components/ui/DateDisplay';

beforeEach(() => {
  vi.useFakeTimers({ toFake: ['Date'] });
  vi.setSystemTime(new Date('2026-09-29T12:00:00'));
});
afterEach(() => vi.useRealTimers());

describe('DateDisplay', () => {
  it('shows a calendar date as a date, even today', () => {
    render(<DateDisplay value="2026-09-29" />);
    expect(screen.getByText('2026. 09. 29.')).toBeInTheDocument();
  });

  it('shows a calendar date from this week as a date too', () => {
    render(<DateDisplay value="2026-09-26" withTime />);
    expect(screen.getByText('2026. 09. 26.')).toBeInTheDocument();
  });

  it('still shows a recent timestamp relative, with the date on hover', () => {
    render(<DateDisplay value="2026-09-29T09:00:00" />);
    const time = screen.getByText(/ezelőtt/);
    expect(time).toHaveAttribute('title', '2026. 09. 29.');
  });

  it('shows an older timestamp as a date', () => {
    render(<DateDisplay value="2026-08-01T09:00:00" withTime />);
    expect(screen.getByText('2026. 08. 01. 09:00')).toBeInTheDocument();
  });
});
