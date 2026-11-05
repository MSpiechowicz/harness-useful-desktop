import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import OfflineBanner from './OfflineBanner.svelte';

describe('OfflineBanner', () => {
  it('says why the database cannot be reached when the server says', () => {
    render(OfflineBanner, { offline: true, message: 'Unreachable', reason: 'cannot reach the PostgreSQL database: timed out' });

    expect(screen.getByRole('status')).toHaveTextContent('cannot reach the PostgreSQL database: timed out');
  });

  it('shows only the message without a reason, and nothing while online', () => {
    const { unmount } = render(OfflineBanner, { offline: true, message: 'Unreachable', reason: null });

    expect(screen.getByRole('status').querySelectorAll('p')).toHaveLength(1);
    unmount();

    render(OfflineBanner, { offline: false, message: 'Unreachable', reason: 'timed out' });
    expect(screen.queryByRole('status')).toBeNull();
  });
});
