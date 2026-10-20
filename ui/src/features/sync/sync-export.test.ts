import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { freshBackend, type Backend } from '../../../tests/ecosystem-backend';
import SyncSection from './SyncSection.svelte';

let backend: Backend;
let clipboard: ReturnType<typeof vi.fn>;

function useScenario(name: string): void {
  window.history.replaceState(null, '', `/?sync=${name}`);
  backend = freshBackend();
}

beforeEach(() => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  clipboard = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, 'clipboard', { value: { writeText: clipboard }, configurable: true });
  useScenario('typical');
});

afterEach(() => {
  window.history.replaceState(null, '', '/');
  vi.restoreAllMocks();
});

async function openExport(): Promise<HTMLElement> {
  render(SyncSection, { id: 'sync' });
  await fireEvent.click(screen.getByRole('button', { name: 'Export…' }));

  return screen.findByRole('dialog', { name: 'Export setup' });
}

describe('Export', () => {
  it('starts with everything ticked, grouped by harness', async () => {
    const dialog = await openExport();

    await within(dialog).findByText('harness-useful-skills');

    const boxes = within(dialog).getAllByRole('checkbox');
    const items = boxes.filter((box) => !/^Select all for/.test(box.getAttribute('aria-label') ?? ''));

    // Twelve items and one "select all" per harness, everything ticked.
    expect(items).toHaveLength(12);
    expect(boxes).toHaveLength(15);
    expect(boxes.every((box) => box.getAttribute('aria-checked') === 'true')).toBe(true);
    expect(within(dialog).getByRole('region', { name: 'OMP' })).toBeInTheDocument();
    expect(within(dialog).getByRole('region', { name: 'PI' })).toBeInTheDocument();
    expect(within(dialog).getByRole('region', { name: 'OPENCODE' })).toBeInTheDocument();
    expect(within(dialog).getAllByText('Runs a program')).toHaveLength(3);
  });

  it('lists the OpenCode plugin by its pinned name, with its servers and what stays here', async () => {
    const dialog = await openExport();
    const section = within(await within(dialog).findByRole('region', { name: 'OPENCODE' }));

    expect(section.getByText('opencode-wakatime@1.2.3')).toBeInTheDocument();
    expect(section.queryByText(/Version 1\.2\.3/)).not.toBeInTheDocument();
    expect(section.queryByText(/marketplace/i)).not.toBeInTheDocument();
    expect(section.getByText('repo-search')).toBeInTheDocument();
    expect(section.getByText('Needs REPO_TOKEN. Set it after importing.')).toBeInTheDocument();
    expect(section.getByText('docs')).toBeInTheDocument();
    expect(section.getByText('Sign-in details are not shared.')).toBeInTheDocument();

    const leftOut = within(await within(dialog).findByRole('region', { name: 'Left out' }));

    expect(leftOut.getByText('my-local-plugin')).toBeInTheDocument();
    expect(leftOut.getByText('OPENCODE')).toBeInTheDocument();
  });

  it('copies the code of the ticked items only, and shows it', async () => {
    const dialog = await openExport();

    await within(dialog).findByText('acme-tools');
    await fireEvent.click(within(dialog).getByRole('checkbox', { name: 'Share acme-tools' }));
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Copy code' }));

    const field = await within(dialog).findByRole('textbox', { name: 'Setup code' });

    expect(backend.calls('sync_export')).toHaveLength(1);
    expect(backend.calls('sync_export')[0].selected).toHaveLength(11);
    expect(backend.calls('sync_export')[0].selected).not.toContain('omp:marketplace:acme-tools');
    expect(field).toHaveAttribute('readonly');
    expect((field as HTMLInputElement).value).toMatch(/^hud1:/);
    expect(clipboard).toHaveBeenCalledWith((field as HTMLInputElement).value);
    expect(within(dialog).getByText('The code is not signed. Share it only with people you trust.')).toBeInTheDocument();
  });

  it('forgets the code when the selection changes', async () => {
    const dialog = await openExport();

    await within(dialog).findByText('acme-tools');
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Copy code' }));
    await within(dialog).findByRole('textbox', { name: 'Setup code' });

    await fireEvent.click(within(dialog).getByRole('checkbox', { name: 'Share acme-tools' }));

    expect(within(dialog).queryByRole('textbox', { name: 'Setup code' })).not.toBeInTheDocument();
  });

  it('cannot export with nothing ticked', async () => {
    const dialog = await openExport();

    await within(dialog).findByText('acme-tools');

    for (const box of within(dialog).getAllByRole('checkbox', { name: /^Select all for/ })) {
      await fireEvent.click(box);
    }

    expect(within(dialog).getByRole('button', { name: 'Copy code' })).toBeDisabled();
    expect(within(dialog).getByRole('button', { name: 'Save file…' })).toBeDisabled();
  });

  it('ticks and unticks a whole harness with its select all', async () => {
    const dialog = await openExport();

    await within(dialog).findByText('acme-tools');

    const all = within(dialog).getByRole('checkbox', { name: 'Select all for OMP' });
    const omp = within(dialog.querySelector('section[aria-labelledby$="-omp"]') as HTMLElement);

    await fireEvent.click(all);
    expect(all.getAttribute('aria-checked')).toBe('false');
    expect(omp.getAllByRole('checkbox').filter((box) => box !== all).every((box) => box.getAttribute('aria-checked') === 'false')).toBe(true);

    // The other harness keeps its ticks, so the export is still possible.
    expect(within(dialog).getByRole('button', { name: 'Copy code' })).toBeEnabled();

    await fireEvent.click(all);
    expect(omp.getAllByRole('checkbox').every((box) => box.getAttribute('aria-checked') === 'true')).toBe(true);
  });

  it('saves a file through the native dialog and says where, or that nothing was saved', async () => {
    const dialog = await openExport();

    await within(dialog).findByText('acme-tools');
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save file…' }));

    expect(await within(dialog).findByText('Saved to /home/demo/Documents/harness-setup-2026-10-15.json')).toBeInTheDocument();
    expect(backend.calls('sync_export_save')[0].selected).toHaveLength(12);
  });

  it('says so when the save dialog was closed', async () => {
    useScenario('cancelled');
    const dialog = await openExport();

    await within(dialog).findByText('acme-tools');
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save file…' }));

    expect(await within(dialog).findByText('No file was saved.')).toBeInTheDocument();
  });

  it('lists what is left out with its reason, never a value', async () => {
    useScenario('hostile');
    const dialog = await openExport();

    const section = await within(dialog).findByRole('region', { name: 'Left out' });
    const rows = within(section).getAllByRole('listitem');

    expect(rows).toHaveLength(7);
    expect(within(section).getByText('Contains a secret')).toBeInTheDocument();
    expect(within(section).getByText('Has credentials')).toBeInTheDocument();
    expect(within(section).getByText('Local path')).toBeInTheDocument();
    expect(within(section).getByText('Project only')).toBeInTheDocument();
    expect(within(section).getByText('Too many items')).toBeInTheDocument();
    expect(within(section).queryByRole('checkbox')).not.toBeInTheDocument();
    expect(section.textContent).not.toMatch(/ghp_|Bearer|https?:\/\//);
  });

  it('says there is nothing to share on an empty machine', async () => {
    useScenario('empty');
    const dialog = await openExport();

    expect(await within(dialog).findByText('Nothing to share yet')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Copy code' })).toBeDisabled();
    await waitFor(() => expect(within(dialog).queryByRole('checkbox')).not.toBeInTheDocument());
  });
});
