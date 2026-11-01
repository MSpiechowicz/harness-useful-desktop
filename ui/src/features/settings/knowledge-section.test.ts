import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import type { KnowledgeCheck } from '$lib/ipc/bindings/KnowledgeCheck';
import type { KnowledgeProblem } from '$lib/ipc/bindings/KnowledgeProblem';
import { ipc } from '$lib/ipc/client';
import { harness } from '$lib/stores/harness.svelte';
import { knowledgeRemote } from '$lib/stores/knowledgeRemote.svelte';
import { knowledgeStatus } from '$lib/stores/knowledgeStatus.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import { checkText, KNOWLEDGE_REMOTE, savedConnection } from './knowledge-remote';
import SettingsView from './SettingsView.svelte';

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  Element.prototype.scrollIntoView = vi.fn();
  await harness.load();
});

beforeEach(() => {
  prefs.value = samplePrefs({ motion: 'off' });
  prefs.dismissKnowledgeManualRemovals();
  knowledgeRemote.value = null;
  knowledgeStatus.value = null;
});

afterEach(() => {
  vi.restoreAllMocks();
});

function section(): HTMLElement {
  return screen.getByRole('region', { name: 'Knowledge' });
}

/** The check a database answers with, for what it lacks. */
function lacking(problem: KnowledgeProblem | null, message: string | null = null): KnowledgeCheck {
  return { ok: false, backend: 'postgres', problem, message, capabilities: null, model: null };
}

describe('the words of a database check', () => {
  const PROBLEMS: Exclude<KnowledgeProblem, 'other'>[] = ['noMongot', 'noPgvector', 'pgvectorNoPrivilege', 'pgvectorTooOld', 'libsqlNoVector', 'noFts5', 'indexBuilding', 'unreachable', 'auth'];

  it('says what each missing thing is and how to put it right, in its own sentence', () => {
    const texts = PROBLEMS.map((problem) => checkText(lacking(problem, 'the server said something')));

    expect(new Set(texts).size).toBe(PROBLEMS.length);
    expect(texts.every((text) => !text.includes('the server said something'))).toBe(true);
    expect(checkText(lacking('noPgvector'))).toMatch(/pgvector extension is not installed/);
    expect(checkText(lacking('noMongot'))).toMatch(/search service \(mongot\)/);
  });

  it('shows the server’s own line for a problem the app has no sentence for', () => {
    expect(checkText(lacking('other', 'disk is full'))).toBe('The database is not ready: disk is full');
    expect(checkText(lacking(null, null))).toBe('The database is not ready: No details were given.');
  });

  it('says a database that has everything is ready', () => {
    expect(checkText({ ok: true, backend: 'postgres', problem: null, message: null, capabilities: null, model: null })).toBe('The database has everything Harness Knowledge needs.');
  });

  it('offers PostgreSQL, MongoDB and libSQL, and never Amazon DocumentDB', () => {
    expect(KNOWLEDGE_REMOTE.kinds).toEqual(['postgres', 'mongodb', 'libsql']);
    expect(savedConnection({ enabled: true, kind: 'documentdb', urlRedacted: 'mongodb://x@h/db', hasToken: false, embedderSupported: true, repositoryDocuments: 'remote', globalDocuments: 'remote', routingSupported: true, configPath: '', problem: null }).kind).toBeNull();
  });
});

describe('Knowledge section', () => {
  it('is its own entry in the section nav and holds the switch, the vector search and remote knowledge', async () => {
    render(SettingsView);
    const nav = screen.getByRole('navigation', { name: 'Settings sections' });

    await fireEvent.click(within(nav).getByRole('button', { name: 'Knowledge' }));

    expect(within(nav).getByRole('button', { name: 'Knowledge' })).toHaveAttribute('aria-current', 'true');
    expect(within(section()).getByRole('switch', { name: 'Project knowledge' })).toBeInTheDocument();
    expect(await within(section()).findByRole('switch', { name: 'Remote knowledge' })).toBeInTheDocument();
  });

  it('turns Project knowledge on and off through its own command, leaving memory alone', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    render(SettingsView);

    await fireEvent.click(within(section()).getByRole('switch', { name: 'Project knowledge' }));

    await waitFor(() => expect(prefs.value?.knowledgeEnabled).toBe(false));
    expect(invoke).toHaveBeenCalledWith('knowledge_set_enabled', { enabled: false });
    expect(invoke).not.toHaveBeenCalledWith('memory_set_enabled', expect.anything());
    expect(prefs.value?.memoryEnabled).toBe(true);
  });

  it('installs a missing Harness Knowledge right here after the user confirmed what it runs', async () => {
    render(SettingsView);

    expect(await within(section()).findByText('Harness Knowledge is not installed')).toBeInTheDocument();
    await fireEvent.click(within(section()).getByRole('button', { name: 'Install' }));

    const confirmation = await screen.findByRole('dialog', { name: 'Install Harness Knowledge?' });
    expect(within(confirmation).getByText(/harness-useful-knowledge/)).toBeInTheDocument();
    await fireEvent.click(within(confirmation).getByRole('button', { name: 'Install' }));

    await waitFor(() => expect(within(section()).queryByText('Harness Knowledge is not installed')).toBeNull(), { timeout: 3000 });
    expect(knowledgeStatus.value?.availability.type).toBe('available');
  });

  it('tests a remote knowledge database and explains what it lacks, then saves one that has everything', async () => {
    const invoke = vi.spyOn(ipc, 'invoke');
    render(SettingsView);

    const toggle = await within(section()).findByRole('switch', { name: 'Remote knowledge' });
    await waitFor(() => expect(toggle).toBeEnabled());
    await fireEvent.click(toggle);

    await fireEvent.click(await within(section()).findByRole('button', { name: /Database: PostgreSQL/ }));
    expect(screen.getAllByRole('menuitemradio').map((item) => item.textContent?.trim())).toEqual(['PostgreSQL', 'MongoDB', 'Remote SQLite (libSQL, Turso)']);
    await fireEvent.keyDown(document.body, { key: 'Escape' });

    const url = within(section()).getByLabelText('Connection URL');
    await fireEvent.input(url, { target: { value: 'postgres://me:pw@db.nopgvector.example/knowledge' } });
    await fireEvent.click(within(section()).getByRole('button', { name: 'Test connection' }));
    expect(await within(section()).findByText(/The pgvector extension is not installed/)).toBeInTheDocument();

    await fireEvent.input(url, { target: { value: 'postgres://me:pw@db.ok.example/knowledge' } });
    await fireEvent.click(within(section()).getByRole('button', { name: 'Test connection' }));
    expect(await within(section()).findByText('The database has everything Harness Knowledge needs.')).toBeInTheDocument();

    await fireEvent.click(within(section()).getByRole('button', { name: 'Save' }));
    expect(await within(section()).findByText('Saved. Remote knowledge is on.')).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith('knowledge_remote_set', { enabled: true, kind: 'postgres', url: 'postgres://me:pw@db.ok.example/knowledge', token: null, repositoryDocuments: 'remote', globalDocuments: 'remote' });
    expect(knowledgeRemote.value?.urlRedacted).toBe('postgres://me@db.ok.example/knowledge');
  });
});
