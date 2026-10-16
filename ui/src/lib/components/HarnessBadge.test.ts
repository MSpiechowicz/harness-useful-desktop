import { render } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import HarnessBadge from './HarnessBadge.svelte';

describe('HarnessBadge', () => {
  it('shows a short name as it is and reduces a long one to its capitals', () => {
    const omp = render(HarnessBadge, { props: { info: { id: 'omp', shortName: 'OMP' } } });
    expect(omp.container.textContent?.trim()).toBe('OMP');

    const opencode = render(HarnessBadge, { props: { info: { id: 'opencode', shortName: 'OpenCode' } } });
    expect(opencode.container.textContent?.trim()).toBe('OC');
  });

  it('reduces an upper-cased id, the stand-in while the harness list loads, to two letters', () => {
    const { container } = render(HarnessBadge, { props: { info: { id: 'opencode', shortName: 'OPENCODE' } } });

    expect(container.textContent?.trim()).toBe('OP');
  });

  it('colours each harness with its own accent', () => {
    const { container } = render(HarnessBadge, { props: { info: { id: 'opencode', shortName: 'OpenCode' } } });

    expect(container.firstElementChild).toHaveClass('bg-opencode-wash', 'text-opencode');
  });
});
