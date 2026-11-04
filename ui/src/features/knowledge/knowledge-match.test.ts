import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import type { KnowledgeSimilar } from '$lib/ipc/bindings/KnowledgeSimilar';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { knowledgeImport } from '$lib/stores/knowledgeImport.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { calls } from '../../../tests/doc-support';
import { startMockApp } from '../../../tests/mock-app';
import { fakeKnowledge, resetKnowledge, routedRemote, show } from './knowledge-view-support';

const DIR = '/home/demo/Documents';

beforeAll(async () => {
  await startMockApp(fixtures.recentProjects[0].path);
});

beforeEach(() => {
  resetKnowledge();
});

afterEach(() => {
  nav.go('chat');
  toasts.items = [];
  vi.restoreAllMocks();
});

async function stage(what: 'Choose files' | 'Choose a folder' = 'Choose files'): Promise<void> {
  await fireEvent.click(screen.getAllByRole('button', { name: 'Add documents' })[0]);
  await fireEvent.click(await screen.findByRole('button', { name: what }));
  // Where the import goes is read from the routing: until then every match counts as being in another place.
  await waitFor(() => expect(knowledgeImport.landing).not.toBe('unknown'));
}

const table = () => screen.findByRole('table', { name: 'Chosen files and the documents they match' });

/** The row of a file in the match table. */
async function rowOf(file: string): Promise<HTMLElement> {
  const row = within(await table())
    .getAllByRole('row')
    .find((candidate) => within(candidate).queryByText(file) !== null);

  if (!row) {
    throw new Error(`no row for ${file}`);
  }

  return row;
}

describe('the files of an import next to the documents they are versions of', () => {
  it('lists each file with its match, why it matched, and what Start import will do', async () => {
    fakeKnowledge();
    await show();
    await stage();

    const architecture = await rowOf('architecture.md');
    expect(within(architecture).getByText('Architecture overview')).toBeInTheDocument();
    expect(within(architecture).getByText('Remote knowledge · added by Ada Lovelace')).toBeInTheDocument();
    expect(within(architecture).getByRole('button', { name: /architecture\.md: Update/ })).toBeInTheDocument();
    // The reason shows on hover of the document.
    expect(within(architecture).getByTitle('Same path')).toBeInTheDocument();

    expect(within(await rowOf('billing-openapi-v2.yaml')).getByTitle('Mostly the same sections')).toBeInTheDocument();
    expect(within(await rowOf('glossary.md')).getByTitle('Same title')).toBeInTheDocument();

    // A file that could not be compared is added as it is, and says why.
    const scan = await rowOf('scan-of-contract.pdf');
    expect(within(scan).getByText(/Not compared: the PDF has no text layer/)).toBeInTheDocument();
    expect(within(scan).getByText('Add as new')).toBeInTheDocument();
    expect(within(scan).queryByRole('button')).toBeNull();

    // The matched files come first.
    const names = within(await table())
      .getAllByRole('row')
      .slice(1)
      .map((row) => row.querySelector('p')?.textContent);
    expect(names.slice(0, 3)).toEqual(['architecture.md', 'billing-openapi-v2.yaml', 'glossary.md']);

    // A title alone is a guess: the file is added as new unless the user switches it.
    expect(within(await rowOf('glossary.md')).getByRole('button', { name: /glossary\.md: Add as new/ })).toBeInTheDocument();
    expect(screen.getByText('2 will update, 1 will be added')).toBeInTheDocument();
  });

  it('changes the summary with each choice, and starts the import replacing only the files set to Update', async () => {
    const { invoke } = fakeKnowledge();
    await show();
    await stage();

    await fireEvent.click(within(await rowOf('architecture.md')).getByRole('button', { name: /architecture\.md: Update/ }));
    expect(await screen.findByText('Replaces Architecture overview')).toBeInTheDocument();
    expect(screen.getByText('Keeps both documents')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('menuitemradio', { name: /Add as new/ }));

    expect(await screen.findByText('1 will update, 2 will be added')).toBeInTheDocument();
    expect(within(await rowOf('architecture.md')).getByRole('button', { name: /architecture\.md: Add as new/ })).toBeInTheDocument();

    // The glossary matched by its title only: the user can still choose to update it.
    await fireEvent.click(within(await rowOf('glossary.md')).getByRole('button', { name: /glossary\.md: Add as new/ }));
    await fireEvent.click(await screen.findByRole('menuitemradio', { name: /Update/ }));

    expect(await screen.findByText('2 will update, 1 will be added')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Start import' }));
    await screen.findByText('Import finished');

    expect(calls(invoke, 'knowledge_import').at(-1)).toMatchObject({
      options: {
        replacing: [
          { path: `${DIR}/billing-openapi-v2.yaml`, id: 2 },
          { path: `${DIR}/glossary.md`, id: 10 },
        ],
      },
    });
  });

  it('sends nothing to replace when every file is set to be added', async () => {
    const { invoke } = fakeKnowledge();
    await show();
    await stage();

    for (const file of ['architecture.md', 'billing-openapi-v2.yaml']) {
      await fireEvent.click(within(await rowOf(file)).getByRole('button', { name: new RegExp(`${file.replace('.', '\\.')}: Update`) }));
      await fireEvent.click(await screen.findByRole('menuitemradio', { name: /Add as new/ }));
    }

    expect(await screen.findByText('3 will be added')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Start import' }));
    await screen.findByText('Import finished');

    expect((calls(invoke, 'knowledge_import').at(-1) as { options: Record<string, unknown> }).options).not.toHaveProperty('replacing');
  });

  describe('when documents are kept in two places', () => {
    const both = routedRemote('remoteLocal', 'remoteLocal');

    /** Saves the import to this computer: the files' matches are in the team database. */
    async function saveToThisComputer(): Promise<void> {
      await fireEvent.click(within(await screen.findByRole('radiogroup', { name: 'Save to' })).getByRole('radio', { name: 'This computer' }));
    }

    it('updates a same-place match by default and warns about none', async () => {
      const { invoke } = fakeKnowledge('ok', true, both);
      await show();
      await stage();

      expect(await screen.findByText('2 will update, 1 will be added')).toBeInTheDocument();
      expect(screen.queryByTitle(/replace it from its panel/)).toBeNull();

      await fireEvent.click(screen.getByRole('button', { name: 'Start import' }));
      await screen.findByText('Import finished');

      expect(calls(invoke, 'knowledge_import').at(-1)).toMatchObject({ options: { place: 'remote', replacing: [{ id: 1 }, { id: 2 }] } });
    });

    it('adds a file as new by default when its match is in the other place than the import goes to', async () => {
      const { invoke } = fakeKnowledge('ok', true, both);
      await show();
      await stage();
      await saveToThisComputer();

      expect(await screen.findByText('3 will be added')).toBeInTheDocument();

      await fireEvent.click(screen.getByRole('button', { name: 'Start import' }));
      await screen.findByText('Import finished');

      const { options } = calls(invoke, 'knowledge_import').at(-1) as { options: Record<string, unknown> };
      expect(options).toMatchObject({ place: 'local' });
      expect(options).not.toHaveProperty('replacing');
    });

    it('adds a file whose match is kept in the other place as new, with no Update to choose, and says in a tooltip where the match is kept', async () => {
      const { invoke } = fakeKnowledge('ok', true, both);
      await show();
      await stage();
      await saveToThisComputer();

      const architecture = await rowOf('architecture.md');
      expect(within(architecture).getByText('Remote knowledge · added by Ada Lovelace')).toBeInTheDocument();
      expect(within(architecture).queryByRole('button')).toBeNull();
      expect(within(architecture).getByText('Add as new')).toBeInTheDocument();
      expect(within(architecture).getByTitle('Kept in Remote, replace it from its panel')).toBe(within(architecture).getByText('Add as new'));

      await fireEvent.click(screen.getByRole('button', { name: 'Start import' }));
      await screen.findByText('Import finished');

      expect((calls(invoke, 'knowledge_import').at(-1) as { options: Record<string, unknown> }).options).not.toHaveProperty('replacing');
    });
  });

  it('shows the files while the comparison runs, with a skeleton where the documents will be', async () => {
    let answer: (value: KnowledgeSimilar) => void = () => {};
    fakeKnowledge('ok', false, { knowledge_similar: () => new Promise<KnowledgeSimilar>((resolve) => (answer = resolve)) });
    await show();
    await stage();

    const pending = await table();
    expect(pending).toHaveAttribute('aria-busy', 'true');
    expect(within(pending).getByText('architecture.md')).toBeInTheDocument();
    expect(within(pending).queryByText('Architecture overview')).toBeNull();
    expect(within(pending).queryByRole('button')).toBeNull();

    answer({ items: [{ path: `${DIR}/architecture.md`, match: null, error: null }], offline: true });

    await waitFor(() => expect(pending).toHaveAttribute('aria-busy', 'false'));
    expect(screen.getByText('Remote knowledge is unreachable, so only documents on this computer were compared.')).toBeInTheDocument();
  });

  it('lists the files of a folder with no choice, because only files that were chosen can replace a document', async () => {
    const found = {
      id: 3,
      title: 'Standup 28 October (transcript)',
      place: 'remote',
      reason: 'sourceKey',
      sharedChunks: 9,
      chunks: 9,
      updatedAt: new Date().toISOString(),
    };
    const { invoke } = fakeKnowledge('ok', false, {
      knowledge_similar: () => ({ items: [{ path: `${DIR}/team-docs/handbook.md`, match: found, error: null }], offline: false }),
    });
    await show();
    await stage('Choose a folder');

    const row = await rowOf('handbook.md');
    expect(within(row).getByText('Standup 28 October (transcript)')).toBeInTheDocument();
    expect(within(row).queryByRole('button')).toBeNull();
    expect(within(row).getByText('Add as new')).toBeInTheDocument();
    expect(screen.getByText('1 will be added')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Start import' }));
    await screen.findByText('Import finished');

    expect((calls(invoke, 'knowledge_import').at(-1) as { options: Record<string, unknown> }).options).not.toHaveProperty('replacing');
  });

  it('keeps the plain list of files, and asks nothing, of a Harness Knowledge that cannot compare', async () => {
    const { invoke } = fakeKnowledge('older');
    await show();
    await stage();

    expect(await screen.findByRole('list', { name: 'Chosen files and folders' })).toBeInTheDocument();
    expect(screen.queryByRole('table', { name: 'Chosen files and the documents they match' })).toBeNull();
    expect(screen.queryByText(/will update/)).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'Start import' }));
    await screen.findByText('Import finished');

    expect(calls(invoke, 'knowledge_similar')).toHaveLength(0);
    expect((calls(invoke, 'knowledge_import').at(-1) as { options: Record<string, unknown> }).options).not.toHaveProperty('replacing');
  });

  it('lists the files plainly when the comparison fails, and imports them as it always did', async () => {
    const { invoke } = fakeKnowledge('ok', false, {
      knowledge_similar: () => {
        throw new Error('the comparison timed out');
      },
    });
    await show();
    await stage();

    expect(await screen.findByRole('list', { name: 'Chosen files and folders' })).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Start import' }));
    await screen.findByText('Import finished');

    expect((calls(invoke, 'knowledge_import').at(-1) as { options: Record<string, unknown> }).options).not.toHaveProperty('replacing');
  });

  it('compares again when the first link changes, because a title only matches inside its scope', async () => {
    const { invoke } = fakeKnowledge();
    await show();
    await stage();
    await table();
    expect(calls(invoke, 'knowledge_similar')).toHaveLength(1);

    await fireEvent.click(screen.getByRole('button', { name: 'Link a project' }));
    await fireEvent.click(await screen.findByRole('menuitem', { name: /useful\/website/ }));

    await waitFor(() => expect(calls(invoke, 'knowledge_similar')).toHaveLength(2));
    expect(calls(invoke, 'knowledge_similar').at(-1)).toMatchObject({ links: [{ repo: 'git:github.com/useful/website', globs: [] }] });
  });
});
