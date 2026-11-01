<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeRemote } from '$lib/ipc/bindings/KnowledgeRemote';
  import type { MemoryNotesRouting } from '$lib/ipc/bindings/MemoryNotesRouting';

  import { documentHints } from './knowledge-remote';
  import { keptInOptions } from './kept-in';
  import KeptInRow from './KeptInRow.svelte';

  interface Props {
    /** What the installed Harness Knowledge can do. */
    remote: KnowledgeRemote;
    repositoryDocuments: MemoryNotesRouting;
    globalDocuments: MemoryNotesRouting;
    onchange: (scope: 'repositoryDocuments' | 'globalDocuments', routing: MemoryNotesRouting) => void;
  }

  let { remote, repositoryDocuments, globalDocuments, onchange }: Props = $props();

  /** An older Harness Knowledge keeps everything in the remote database: the other places stay listed, greyed out, and the note under the rows says what to do. */
  const options = $derived(keptInOptions(remote.routingSupported ? {} : { local: null, remoteLocal: null }));
  const hints = $derived(documentHints());
  const lockedNote = $derived(t('knowledge.settings.keptIn.env'));
</script>

<KeptInRow
  title={t('knowledge.settings.projectDocs.title')}
  description={t('knowledge.settings.projectDocs.description')}
  {options}
  {hints}
  value={repositoryDocuments}
  locked={remote.repositoryDocumentsEnv ? lockedNote : undefined}
  onchange={(next) => onchange('repositoryDocuments', next)}
/>

<KeptInRow
  title={t('knowledge.settings.globalDocs.title')}
  description={t('knowledge.settings.globalDocs.description')}
  {options}
  {hints}
  value={globalDocuments}
  locked={remote.globalDocumentsEnv ? lockedNote : undefined}
  onchange={(next) => onchange('globalDocuments', next)}
/>

{#if !remote.routingSupported}
  <p class="py-3 text-sm text-fg-3" role="status">{t('knowledge.settings.routing.unsupported')}</p>
{/if}
