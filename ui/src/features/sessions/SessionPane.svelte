<script lang="ts">
  import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { forkInChat } from '$lib/stores/session-actions';
  import { sessionDetail as detail } from '$lib/stores/sessions.svelte';

  import Transcript from './Transcript.svelte';

  interface Props {
    summary: SessionSummary;
    project: string;
    /** The session is being resumed in a chat; forking waits for it. */
    resuming?: boolean;
  }

  let { summary, project, resuming = false }: Props = $props();

  let busy = $state(false);

  // Forking starts a chat, which a harness without chat cannot do.
  const forkable = $derived(harness.canChat(summary.ref.harness));

  async function fork(entryId: string): Promise<void> {
    busy = true;
    await forkInChat(agentSessions, summary.ref, entryId, project);
    busy = false;
  }
</script>

<!-- The header of the session is the view's (`WorkspaceLayout` puts it above this), and its details are the view's side panel:
     the transcript keeps its own scroller and measure. -->
<div class="min-h-0 min-w-0 flex-1">
  <Transcript {detail} {project} busy={busy || resuming} onfork={forkable ? (id) => void fork(id) : undefined} />
</div>
