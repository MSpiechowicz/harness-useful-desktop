<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

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

<Dialog open title={t('chat.bash.title')} description={t('chat.bash.description')} onclose={close}>
  <div class="flex flex-col gap-3">
    <TerminalBlock text="" {command} />
    <p class="text-sm break-all text-fg-3">{t('chat.bash.folder')} <code class="font-mono">{project}</code></p>
  </div>

  {#snippet footer()}
    <Button onclick={close}>{t('chat.bash.cancel')}</Button>
    <Button variant="danger" onclick={run}>{t('chat.bash.run')}</Button>
  {/snippet}
</Dialog>
