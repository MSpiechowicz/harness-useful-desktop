import { render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';

import type { Budget } from '$lib/dashboard/models';
import { dashboard } from '$lib/stores/dashboard.svelte';

import { causeColor } from '../charts/error-colors';
import BudgetsCard from './BudgetsCard.svelte';
import { phraseOf, RANGE_PHRASE } from './ranges';

const budget: Budget = { scope: 'global', period: 'month', project: null, capUsd: 10, spentUsd: 2, usedPercent: 20, projectedUsd: null, status: 'ok' };

describe('names the dashboard sends that are also Object properties', () => {
  afterEach(() => {
    dashboard.budgets.data = null;
  });

  it.each(['constructor', '__proto__'])('shows the budget status %s as its own text', (status) => {
    dashboard.budgets.data = [{ ...budget, status }];

    render(BudgetsCard);

    expect(screen.getByText(status)).toBeTruthy();
  });

  it.each(['constructor', '__proto__'])('paints the failure cause %s gray', (cause) => {
    expect(causeColor(cause)).toBe(causeColor('some other cause'));
  });

  it.each(['constructor', '__proto__', 'toString'])('falls back to the chosen range when the data covers %s', (covered) => {
    expect(phraseOf(covered, '7d')).toBe(RANGE_PHRASE['7d']);
  });

  it('still reads the range the data covers', () => {
    expect(phraseOf('30d', '7d')).toBe(RANGE_PHRASE['30d']);
  });
});
