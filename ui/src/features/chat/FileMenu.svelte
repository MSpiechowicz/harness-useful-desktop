<script lang="ts">
  import File from '@lucide/svelte/icons/file';
  import Folder from '@lucide/svelte/icons/folder';

  import Badge from '$lib/components/Badge.svelte';
  import HighlightedText from '$lib/components/HighlightedText.svelte';
  import type { ProjectFile } from '$lib/ipc/bindings/ProjectFile';
  import { fuzzyMatch } from '$lib/utils/fuzzy';
  import { t } from '$lib/i18n/i18n.svelte';

  import SuggestionMenu from './SuggestionMenu.svelte';

  interface Props {
    id: string;
    files: ProjectFile[];
    query: string;
    active: number;
    onaccept: (file: ProjectFile) => void;
    onhover: (index: number) => void;
  }

  let { id, files, query, active, onaccept, onhover }: Props = $props();

  /** The file or folder name and the folder it sits in. */
  function split(path: string): { name: string; folder: string } {
    const slash = path.lastIndexOf('/');

    return { name: path.slice(slash + 1), folder: slash === -1 ? '' : path.slice(0, slash) };
  }

  /** Matched letters of the name. A query with a folder in it (`src/li`) is matched by its last part. */
  function highlights(name: string): number[] {
    const typed = query.slice(query.lastIndexOf('/') + 1);

    return (fuzzyMatch(query, name) ?? fuzzyMatch(typed, name))?.indices ?? [];
  }
</script>

<SuggestionMenu {id} label={t('chat.files.label')} items={files} {active} rowClass="items-center" itemKey={(file) => file.path} {onaccept} {onhover}>
  {#snippet row(file)}
    {@const { name, folder } = split(file.path)}
    {#if file.isDir}
      <Folder size={16} class="shrink-0 text-fg-3" aria-hidden="true" />
    {:else}
      <File size={16} class="shrink-0 text-fg-3" aria-hidden="true" />
    {/if}
    <span class="max-w-3/5 shrink-0 truncate text-fg">{#if file.isDir}<span class="sr-only">{t('chat.files.folder')}{' '}</span>{/if}<HighlightedText text={name} indices={highlights(name)} /></span>
    <span class="min-w-0 flex-1 truncate text-left text-fg-3" dir="rtl"><bdi>{folder}</bdi></span>
    {#if file.changed}
      <Badge class="shrink-0">{t('chat.files.changed')}</Badge>
    {/if}
  {/snippet}
</SuggestionMenu>
