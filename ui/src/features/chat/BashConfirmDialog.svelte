<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';

  import TerminalBlock from './TerminalBlock.svelte';

  interface Props {
    session: ChatSession;
    command: string;
  }

  let { session, command }: Props = $props();

  const project = $derived(session.info.project);

  function close(): void {
    session.ui.confirmBash = null;
  }

  function run(): void {
    // `command` follows the session state, which `close` clears.
    const confirmed = command;

    close();
    session.draft = '';
    void session.runBash(confirmed);
  }
</script>

<Dialog open title="Run this command?" description="It runs in your project folder with your permissions, outside the agent's approval flow." onclose={close}>
  <div class="flex flex-col gap-3">
    <TerminalBlock text="" {command} />
    <p class="text-sm break-all text-fg-3">Folder: <code class="font-mono">{project}</code></p>
  </div>

  {#snippet footer()}
    <Button onclick={close}>Cancel</Button>
    <Button variant="danger" onclick={run}>Run command</Button>
  {/snippet}
</Dialog>
