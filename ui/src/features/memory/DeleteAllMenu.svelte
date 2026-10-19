<script lang="ts">
  import FileCode from '@lucide/svelte/icons/file-code';
  import NotebookText from '@lucide/svelte/icons/notebook-text';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import type { Snippet } from 'svelte';

  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import type { IconComponent } from '$lib/components/icon';
  import MenuItem from '$lib/components/MenuItem.svelte';
  import MoreMenu from '$lib/components/MoreMenu.svelte';
  import { type PluralBase, t, tn } from '$lib/i18n/i18n.svelte';
  import type { MessageKey } from '$lib/i18n/locales/en';
  import { IpcFailure } from '$lib/ipc/transport';
  import { noteList } from '$lib/stores/memoryNotes.svelte';
  import { scriptList } from '$lib/stores/memoryScripts.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { toasts } from '$lib/stores/toasts.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';

  import { deleteAll, type DeleteScope, type MemorySection, readScope, STAY_TEXT, teamLine } from './delete-all';
  import { totalCount } from './note-format';

  interface Props {
    /** The list on screen: the menu acts on it. */
    section: MemorySection;
    /** More items above "delete all", e.g. Notes' import. They get the menu's `close`. */
    extra?: Snippet<[{ close: () => void }]>;
  }

  interface Text {
    icon: IconComponent;
    action: MessageKey;
    title: PluralBase;
    body: MessageKey;
    confirm: MessageKey;
    failed: MessageKey;
    count: PluralBase;
  }

  let { section, extra }: Props = $props();

  const TEXT: Record<MemorySection, Text> = {
    notes: {
      icon: NotebookText,
      action: 'memory.deleteAll.notes.action',
      title: 'memory.deleteAll.notes.title',
      body: 'memory.deleteAll.notes.body',
      confirm: 'memory.deleteAll.notes.confirm',
      failed: 'memory.deleteAll.notes.failed',
      count: 'memory.notes.count',
    },
    scripts: {
      icon: FileCode,
      action: 'memory.deleteAll.scripts.action',
      title: 'memory.deleteAll.scripts.title',
      body: 'memory.deleteAll.scripts.body',
      confirm: 'memory.deleteAll.scripts.confirm',
      failed: 'memory.deleteAll.scripts.failed',
      count: 'memory.scripts.count',
    },
  };

  const path = $derived(project.active?.path ?? null);

  // How many the list says there are; null while it was not read or cannot be.
  const listed = $derived.by<number | null>(() => {
    if (section === 'notes') {
      return noteList.data?.availability.kind === 'available' ? totalCount(noteList.data.kinds) : null;
    }

    const page = scriptList.data;

    return page?.availability.kind === 'available' ? page.groups.repo + page.groups.global : null;
  });
  const narrowed = $derived.by(() => {
    if (section === 'notes') {
      return viewState.notesQuery.trim() !== '' || viewState.notesScope !== null || viewState.notesKind !== null;
    }

    return viewState.scriptsQuery.trim() !== '' || viewState.scriptsScope !== null || viewState.scriptsStatus !== null || viewState.scriptsGroup !== null;
  });

  // A search or a filter that leaves nothing still has something behind it to delete, so only an empty list that is not narrowed is disabled.
  const disabled = $derived(listed === null || (listed === 0 && !narrowed));

  // What the open question is about, kept after it closes so it fades out showing the same words.
  let target = $state<{ section: MemorySection; scope: DeleteScope }>({ section: 'notes', scope: { total: 0, team: 0, stay: 0, exact: true } });
  let open = $state(false);
  let asking = false;

  const text = $derived(TEXT[target.section]);

  /** Reads what is there first, as the list on screen may be narrowed by a search, so the question names the real number. */
  async function ask(close: () => void): Promise<void> {
    close();

    if (asking) {
      return;
    }

    asking = true;

    try {
      const scope = (await readScope(section, path)) ?? { total: listed ?? 0, team: 0, stay: 0, exact: false };

      if (scope.total > 0) {
        target = { section, scope };
        open = true;
      } else if (scope.stay > 0) {
        // Only remote entries that cannot be deleted here are left: say so instead of asking about nothing.
        toasts.push({ title: tn(STAY_TEXT[section].onlyTeam, scope.stay), tone: 'info' });
      }
    } finally {
      asking = false;
    }
  }

  async function remove(): Promise<void> {
    try {
      await deleteAll(target.section, path);
    } catch (error) {
      throw new Error(`${t(text.failed)}: ${IpcFailure.from(error).message}`);
    }
  }
</script>

<MoreMenu label={t('memory.menu.label')} panelClass="w-56">
  {#snippet children({ close })}
    {@render extra?.({ close })}
    <MenuItem tone="danger" icon={Trash2} {disabled} onclick={() => void ask(close)}>{t(TEXT[section].action)}</MenuItem>
  {/snippet}
</MoreMenu>

<ConfirmDialog
  bind:open
  title={tn(text.title, target.scope.total)}
  description={t(text.body)}
  subject={{ icon: text.icon, name: tn(text.count, target.scope.total), note: teamLine(target.section, target.scope) }}
  confirmLabel={t(text.confirm)}
  cancelLabel={t('memory.deleteAll.cancel')}
  onconfirm={remove}
/>
