import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { LOADING_DELAY_MS } from '$lib/design/motion';

import LoadingSpinner from './LoadingSpinner.svelte';

function wrapper(): HTMLElement {
  return screen.getByRole('status', { name: 'Loading hooks' }).parentElement as HTMLElement;
}

describe('LoadingSpinner', () => {
  it('names what is being read and waits before it shows, so a quick read does not blink', () => {
    render(LoadingSpinner, { label: 'Loading hooks' });

    expect(wrapper().style.animationDelay).toBe(`${LOADING_DELAY_MS}ms`);
  });

  it('shows at once when the caller has already waited', () => {
    render(LoadingSpinner, { label: 'Loading hooks', delay: 0 });

    expect(wrapper().style.animationDelay).toBe('0ms');
  });
});
