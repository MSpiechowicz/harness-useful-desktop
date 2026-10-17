<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import { noteList } from '$lib/stores/memoryNotes.svelte';
  import { scriptList } from '$lib/stores/memoryScripts.svelte';

  import MemoryInstall from './MemoryInstall.svelte';

  // A Harness Memory that cannot combine filters still works: a quiet line at the end of the filter row says what an update adds, and does it.
  async function reread(): Promise<void> {
    await Promise.all([noteList.refresh({ quiet: true }), scriptList.refresh({ quiet: true })]);
  }
</script>

<p class="min-w-0 truncate text-sm text-fg-3 max-[1100px]:hidden">{t('memory.filter.updateHint')}</p>

<MemoryInstall update inline onfinished={reread} />
