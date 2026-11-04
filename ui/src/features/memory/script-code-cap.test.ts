import { render } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import ScriptCode from './ScriptCode.svelte';

const LONG = Array.from({ length: 120 }, (_, row) => `echo "line ${row}"`).join('\n');

describe('the code of a script', () => {
  it('scrolls inside a block capped at the code height token, under a header outside the scroller', () => {
    const { container } = render(ScriptCode, { body: LONG, language: 'bash' });
    const pre = container.querySelector('pre');
    const caption = container.querySelector('figcaption');

    expect(pre).toHaveClass('max-h-[var(--code-max-h)]', 'overflow-y-auto');
    expect(pre).toHaveClass('whitespace-pre-wrap', '[overflow-wrap:anywhere]');
    expect(pre).toHaveAttribute('tabindex', '0');
    expect(caption).not.toBeNull();
    expect(pre?.contains(caption)).toBe(false);
  });

  it('caps the plain block of a script with invisible characters the same way', () => {
    const { container } = render(ScriptCode, { body: `${LONG}‮`, language: 'bash' });

    expect(container.querySelector('pre')).toHaveClass('max-h-[var(--code-max-h)]', 'overflow-y-auto');
  });
});
