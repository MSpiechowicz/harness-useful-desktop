import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';

import { i18n } from '$lib/i18n/i18n.svelte';

import { MEMORY_REMOTE } from './memory-remote';
import { RemoteConnectionDraft, type SavedConnection } from './remote-connection.svelte';
import type { RemoteDatabaseConfig } from './remote-database';
import RemoteDatabasePanel from './RemoteDatabasePanel.svelte';

type OtherKind = 'postgres' | 'mongodb' | 'libsql';

/** Another Harness family member's databases: the same words, three of the four kinds. */
const OTHER: RemoteDatabaseConfig<OtherKind> = {
  ...MEMORY_REMOTE,
  kinds: ['postgres', 'mongodb', 'libsql'],
  defaultKind: 'postgres',
  tokenKinds: ['libsql'],
};

const SAVED: SavedConnection<OtherKind> = { enabled: false, kind: null, urlRedacted: null, hasToken: false, configPath: '~/.config/other/config.toml', problem: null };

function draftFor(probe = vi.fn(async () => ({ ok: true, message: 'Connected.' }))) {
  return { draft: new RemoteConnectionDraft(OTHER, () => SAVED, probe, () => {}), probe };
}

describe('RemoteDatabasePanel with another product config', () => {
  beforeAll(async () => {
    await i18n.load('en');
  });

  it('lists only the databases of its config', async () => {
    const { draft } = draftFor();

    render(RemoteDatabasePanel<OtherKind>, { draft, onsave: () => {} });
    await fireEvent.click(screen.getByRole('button', { name: /Database: PostgreSQL/ }));

    expect(screen.getAllByRole('menuitemradio').map((item) => item.textContent?.trim())).toEqual(['PostgreSQL', 'MongoDB', 'Remote SQLite (libSQL, Turso)']);
  });

  it('tests the typed connection with the token only for a database that takes one', async () => {
    const { draft, probe } = draftFor();

    render(RemoteDatabasePanel<OtherKind>, { draft, onsave: () => {} });
    await fireEvent.input(screen.getByLabelText('Connection URL'), { target: { value: 'postgres://me@db.example/mem' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Test connection' }));

    await waitFor(() => expect(screen.getByText('Connected.')).toBeInTheDocument());
    expect(probe).toHaveBeenCalledWith({ kind: 'postgres', url: 'postgres://me@db.example/mem', token: null });
  });

  it('keeps Save off until the connection is usable and shows what does not fit the database', async () => {
    const { draft } = draftFor();
    const onsave = vi.fn();

    render(RemoteDatabasePanel<OtherKind>, { draft, onsave });

    const field = screen.getByLabelText('Connection URL');

    await fireEvent.input(field, { target: { value: 'mongodb://db.example/mem' } });
    await fireEvent.blur(field);

    expect(screen.getByRole('alert')).toHaveTextContent('A PostgreSQL URL starts with');
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();

    await fireEvent.input(field, { target: { value: 'postgres://me@db.example/mem' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(onsave).toHaveBeenCalledOnce();
  });
});
