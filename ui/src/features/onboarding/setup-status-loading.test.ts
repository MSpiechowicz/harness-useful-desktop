import { render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';

import SetupStatus from './SetupStatus.svelte';

describe('SetupStatus', () => {
  it('shows a spinner, keeping the height it was given, while the machine is read', () => {
    render(SetupStatus, { placeholder: 'h-72', children: createRawSnippet(() => ({ render: () => '<p>read</p>' })) });

    const spinner = screen.getByRole('status', { name: 'Reading your setup' });

    expect(spinner.parentElement).toHaveClass('h-72');
    expect(document.querySelector('.shimmer')).toBeNull();
  });
});
