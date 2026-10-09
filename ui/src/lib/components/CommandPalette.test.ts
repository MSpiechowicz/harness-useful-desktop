import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import CommandPalette from './CommandPalette.svelte';
import type { PaletteCommand } from './palette';

function commands(run = vi.fn()): PaletteCommand[] {
  return [
    { id: 'go-chat', label: 'Go to Chat', group: 'Navigate', run },
    { id: 'go-settings', label: 'Go to Settings', group: 'Navigate', run },
    { id: 'theme-dark', label: 'Theme: Dark', group: 'Appearance', keywords: 'color scheme', run },
    { id: 'disabled', label: 'Theme: Light', group: 'Appearance', disabled: true, run },
  ];
}

async function openPalette(list: PaletteCommand[]) {
  render(CommandPalette, { props: { open: true, commands: list } });

  return screen.findByRole('combobox', { name: 'Search commands' });
}

function optionNames(): string[] {
  return screen.getAllByRole('option').map((option) => option.textContent?.trim() ?? '');
}

describe('CommandPalette', () => {
  it('lists commands grouped, with the first one active', async () => {
    const input = await openPalette(commands());

    expect(screen.getAllByRole('group').map((group) => group.getAttribute('aria-label'))).toEqual(['Navigate', 'Appearance']);
    expect(screen.getAllByRole('option')[0]).toHaveAttribute('aria-selected', 'true');
    expect(input).toHaveAttribute('aria-activedescendant', screen.getAllByRole('option')[0].id);
  });

  it('filters fuzzily by label and by hidden keywords', async () => {
    const input = await openPalette(commands());

    await fireEvent.input(input, { target: { value: 'gtst' } });
    expect(optionNames()).toEqual(['Go to Settings']);

    await fireEvent.input(input, { target: { value: 'color' } });
    expect(optionNames()).toEqual(['Theme: Dark']);
  });

  it('shows an empty message when nothing matches', async () => {
    const input = await openPalette(commands());

    await fireEvent.input(input, { target: { value: 'zzzz' } });

    expect(screen.queryAllByRole('option')).toHaveLength(0);
    expect(screen.getByText('No matching commands')).toBeInTheDocument();
  });

  it('moves the selection with the arrow keys (wrapping) and runs the active command on Enter', async () => {
    const run = vi.fn();
    const list = commands();
    list[1].run = run;
    const input = await openPalette(list);

    await fireEvent.keyDown(input, { key: 'ArrowDown' });
    expect(screen.getAllByRole('option')[1]).toHaveAttribute('aria-selected', 'true');

    await fireEvent.keyDown(input, { key: 'ArrowUp' });
    await fireEvent.keyDown(input, { key: 'ArrowUp' });
    expect(screen.getAllByRole('option')[3]).toHaveAttribute('aria-selected', 'true');

    await fireEvent.keyDown(input, { key: 'ArrowDown' });
    await fireEvent.keyDown(input, { key: 'ArrowDown' });
    await fireEvent.keyDown(input, { key: 'Enter' });

    expect(run).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });

  it('runs a command when it is clicked', async () => {
    const run = vi.fn();
    await openPalette(commands(run));

    await fireEvent.click(screen.getByRole('option', { name: 'Theme: Dark' }));

    expect(run).toHaveBeenCalledTimes(1);
  });

  it('does not run disabled commands', async () => {
    const run = vi.fn();
    await openPalette(commands(run));

    await fireEvent.click(screen.getByRole('option', { name: 'Theme: Light' }));

    expect(run).not.toHaveBeenCalled();
    expect(screen.getByRole('dialog')).toBeInTheDocument();
  });

  it('closes on Escape without running anything', async () => {
    const run = vi.fn();
    const input = await openPalette(commands(run));

    await fireEvent.keyDown(input, { key: 'Escape' });

    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(run).not.toHaveBeenCalled();
  });
});
