import { fireEvent, render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';

import Tooltip from './Tooltip.svelte';

const trigger = createRawSnippet(() => ({ render: () => '<button type="button">Trigger</button>' }));

describe('Tooltip', () => {
  it('renders outside a clipping ancestor so an overflow-hidden dialog cannot cut it off', async () => {
    const clipper = document.createElement('div');
    clipper.style.overflow = 'hidden';
    document.body.append(clipper);

    render(Tooltip, { props: { text: 'Close shortcuts', children: trigger }, target: clipper });
    await fireEvent.pointerEnter(screen.getByRole('button', { name: 'Trigger' }).parentElement as HTMLElement);

    const tooltip = await screen.findByRole('tooltip');

    expect(tooltip).toHaveTextContent('Close shortcuts');
    expect(clipper).not.toContainElement(tooltip);
    expect(tooltip.parentElement).toBe(document.body);
  });
});
