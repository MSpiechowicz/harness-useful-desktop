<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeRemote } from '$lib/ipc/bindings/KnowledgeRemote';
  import type { MemoryNotesRouting } from '$lib/ipc/bindings/MemoryNotesRouting';
  import { knowledgeRemote } from '$lib/stores/knowledgeRemote.svelte';
  import { uid } from '$lib/utils/ids';

  import { documentConfirmCopy, documentEffects, KNOWLEDGE_REMOTE, savedConnection, toTested, type DocumentScope } from './knowledge-remote';
  import KnowledgeKeptIn from './KnowledgeKeptIn.svelte';
  import { REMOTE_SUBHEADING } from './remote-database';
  import { RemoteConnectionDraft } from './remote-connection.svelte';
  import RemoteDatabasePanel from './RemoteDatabasePanel.svelte';
  import RoutingChangeConfirm from './RoutingChangeConfirm.svelte';
  import type { RoutingEffect } from './routing-change';

  interface Props {
    /** What the backend has saved. The form starts from it and keeps its own draft while it is open. */
    remote: KnowledgeRemote;
  }

  let { remote }: Props = $props();

  // The draft starts from what is saved. The secrets start empty and are never read back.
  const draft = new RemoteConnectionDraft(KNOWLEDGE_REMOTE, () => savedConnection(remote), async (probe) => toTested(await knowledgeRemote.test(probe)), () => {});

  // svelte-ignore state_referenced_locally
  let repositoryDocuments = $state<MemoryNotesRouting>(remote.repositoryDocuments);
  // svelte-ignore state_referenced_locally
  let globalDocuments = $state<MemoryNotesRouting>(remote.globalDocuments);
  /** A save that changes where documents are kept waits here for the user to read what it does. */
  let confirming = $state(false);
  let pendingEffects = $state.raw<RoutingEffect<DocumentScope>[]>([]);

  const headingId = `${uid('knowledge-remote')}-kept-in`;
  const routingChanged = $derived(repositoryDocuments !== remote.repositoryDocuments || globalDocuments !== remote.globalDocuments);

  function choose(scope: DocumentScope, next: MemoryNotesRouting): void {
    if (scope === 'repositoryDocuments') {
      repositoryDocuments = next;
    } else {
      globalDocuments = next;
    }

    draft.edited();
  }

  /** What a move would hide or show: said before saving. A first connection has nothing saved to change. */
  function save(): void {
    const effects = remote.enabled ? documentEffects(remote, { repositoryDocuments, globalDocuments }) : [];

    if (effects.length === 0) {
      void commit();

      return;
    }

    pendingEffects = effects;
    confirming = true;
  }

  async function commit(): Promise<void> {
    draft.busy = 'save';
    draft.status = null;

    try {
      await knowledgeRemote.save({
        enabled: true,
        kind: draft.kind,
        url: draft.typedUrl === '' ? null : draft.typedUrl,
        token: draft.tokenToSend(),
        // An older Harness Knowledge refuses a file with the routing table, so it is not sent for it.
        ...(remote.routingSupported ? { repositoryDocuments, globalDocuments } : {}),
      });
      draft.stored();
    } catch (error) {
      draft.fail(error);
    } finally {
      draft.busy = null;
    }
  }
</script>

<RemoteDatabasePanel {draft} otherChanges={routingChanged} onsave={save}>
  {#snippet sections()}
    <section aria-labelledby={headingId}>
      <h3 id={headingId} class={REMOTE_SUBHEADING}>{t('settings.memoryRemote.section.keptIn')}</h3>
      <div class="divide-y divide-line">
        <KnowledgeKeptIn {remote} {repositoryDocuments} {globalDocuments} onchange={choose} />
      </div>
    </section>
  {/snippet}
</RemoteDatabasePanel>

<RoutingChangeConfirm bind:open={confirming} effects={pendingEffects} copy={documentConfirmCopy()} onconfirm={() => void commit()} />
