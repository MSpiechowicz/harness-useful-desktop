<script lang="ts">
  import Power from '@lucide/svelte/icons/power';

  import Button from '$lib/components/Button.svelte';
  import type { CrashInfo } from '$lib/chat/types';

  import TerminalBlock from './TerminalBlock.svelte';

  interface Props {
    crash: CrashInfo;
    onrestart: () => void;
    restarting: boolean;
  }

  let { crash, onrestart, restarting }: Props = $props();

  const reason = $derived(crash.code !== null ? `code ${crash.code}` : crash.signal !== null ? `signal ${crash.signal}` : 'no exit status');
</script>

<section role="alert" aria-label="Agent stopped" class="chat-col mb-2">
  <div class="flex flex-col gap-3 rounded-xl border border-danger-edge bg-danger-wash bg-clip-padding px-4 py-3.5">
    <div class="flex items-center gap-3">
      <Power size={18} class="shrink-0 text-danger" aria-hidden="true" />
      <div class="min-w-0 flex-1">
        <h3 class="text-base font-semibold text-fg">The agent stopped unexpectedly</h3>
        <p class="text-sm text-fg-2">It exited with {reason}. Your messages above are kept; restarting resumes the conversation.</p>
      </div>
      <Button variant="primary" size="sm" loading={restarting} onclick={onrestart}>Restart session</Button>
    </div>

    {#if crash.stderr.length > 0}
      <div class="flex flex-col gap-1">
        <p class="text-xs font-medium tracking-wide text-fg-3 uppercase">Last output</p>
        <TerminalBlock text={crash.stderr.join('\n')} />
      </div>
    {/if}
  </div>
</section>
