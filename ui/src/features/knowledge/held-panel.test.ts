import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { KnowledgeHeldItem } from '$lib/ipc/bindings/KnowledgeHeldItem';

import HeldPanel from './HeldPanel.svelte';

const BASE: KnowledgeHeldItem = {
  id: '0b6f2f0e-7f63-4c0e-9d1a-0d5a4a0a7e11',
  title: 'Retry policy',
  format: 'markdown',
  name: 'retry.md',
  bytes: 40,
  text: 'Retry three times.',
  links: [{ repo: 'git:github.com/acme/widget', globs: ['src/**', 'src/**'] }],
  client: 'omp',
  session: null,
  projectPath: null,
  repoKey: null,
  sourcePath: null,
  replaces: null,
  revision: 'a'.repeat(64),
  createdAt: '2026-10-30T10:00:00Z',
};

function show(props: { detail: KnowledgeHeldItem | null; status?: 'ready' | 'error' | 'loading'; item?: KnowledgeHeldItem }) {
  const onapprove = vi.fn();
  const onreject = vi.fn();

  render(HeldPanel, { open: true, inline: true, item: props.item ?? BASE, detail: props.detail, status: props.status ?? 'ready', error: null, busy: false, onretry: () => {}, onapprove, onreject, onclose: () => {} });

  return { onapprove, onreject };
}

describe('deciding on a proposal', () => {
  it('cannot approve it before its whole text was read, and can still reject it', async () => {
    const { onapprove, onreject } = show({ detail: null, status: 'loading' });

    expect(screen.getByRole('button', { name: 'Approve' })).toBeDisabled();

    await fireEvent.click(screen.getByRole('button', { name: 'Reject' }));
    expect(onreject).toHaveBeenCalledTimes(1);
    expect(onapprove).not.toHaveBeenCalled();
  });

  it('cannot approve it when reading the whole text failed', () => {
    show({ detail: null, status: 'error' });

    expect(screen.getByRole('button', { name: 'Approve' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Reject' })).toBeEnabled();
  });

  it('approves the whole text once it is read', async () => {
    const detail = { ...BASE, text: 'Retry three times. Then stop.' };
    const { onapprove } = show({ detail });

    await fireEvent.click(screen.getByRole('button', { name: 'Approve' }));
    expect(onapprove).toHaveBeenCalledWith(detail);
  });

  it('warns about hidden characters only from the whole text, not from the preview', () => {
    const hidden = `Retry‮ three times.`;
    const { unmount } = render(HeldPanel, { open: true, inline: true, item: { ...BASE, text: hidden }, detail: null, status: 'loading', error: null, busy: false, onretry: () => {}, onapprove: () => {}, onreject: () => {}, onclose: () => {} });

    expect(screen.queryByRole('alert')).toBeNull();
    unmount();

    show({ detail: { ...BASE, text: hidden } });
    expect(screen.getByRole('alert')).toHaveTextContent(/invisible character/);
  });

  it('lists the same glob twice without failing', () => {
    show({ detail: BASE });

    expect(screen.getAllByText('src/**')).toHaveLength(2);
  });
});
