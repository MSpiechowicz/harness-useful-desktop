<script lang="ts">
  import type { KnowledgeRemote } from '$lib/ipc/bindings/KnowledgeRemote';
  import { knowledgeRemote } from '$lib/stores/knowledgeRemote.svelte';

  import { KNOWLEDGE_REMOTE, savedConnection, toTested } from './knowledge-remote';
  import { RemoteConnectionDraft } from './remote-connection.svelte';
  import RemoteDatabasePanel from './RemoteDatabasePanel.svelte';

  interface Props {
    /** What the backend has saved. The form starts from it and keeps its own draft while it is open. */
    remote: KnowledgeRemote;
  }

  let { remote }: Props = $props();

  // The draft starts from what is saved. The secrets start empty and are never read back.
  const draft = new RemoteConnectionDraft(KNOWLEDGE_REMOTE, () => savedConnection(remote), async (probe) => toTested(await knowledgeRemote.test(probe)), () => {});

  async function save(): Promise<void> {
    draft.busy = 'save';
    draft.status = null;

    try {
      await knowledgeRemote.save({ enabled: true, kind: draft.kind, url: draft.typedUrl === '' ? null : draft.typedUrl, token: draft.tokenToSend() });
      draft.stored();
    } catch (error) {
      draft.fail(error);
    } finally {
      draft.busy = null;
    }
  }
</script>

<RemoteDatabasePanel {draft} onsave={() => void save()} />
