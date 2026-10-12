<script lang="ts">
  import Power from '@lucide/svelte/icons/power';

  import Button from '$lib/components/Button.svelte';
  import type { CrashInfo } from '$lib/chat/types';
  import { t } from '$lib/i18n/i18n.svelte';

  import TerminalBlock from './TerminalBlock.svelte';

  interface Props {
    crash: CrashInfo;
    onrestart: () => void;
    restarting: boolean;
  }

  let { crash, onrestart, restarting }: Props = $props();

  const reason = $derived(
    crash.code !== null ? t('chat.crash.code', { code: crash.code }) : crash.signal !== null ? t('chat.crash.signal', { signal: crash.signal }) : t('chat.crash.noStatus'),
  );
</script>

<section role="alert" aria-label={t('chat.crash.label')} class="chat-col mb-2">
  <div class="flex flex-col gap-3 rounded-xl border border-danger-edge bg-danger-wash bg-clip-padding px-4 py-3.5">
    <div class="flex items-center gap-3">
      <Power size={18} class="shrink-0 text-danger" aria-hidden="true" />
      <div class="min-w-0 flex-1">
        <h3 class="text-base font-semibold text-fg">{t('chat.crash.title')}</h3>
        <p class="text-sm text-fg-2">{t('chat.crash.body', { reason })}</p>
      </div>
      <Button variant="primary" size="sm" loading={restarting} onclick={onrestart}>{t('chat.crash.restart')}</Button>
    </div>

    {#if crash.stderr.length > 0}
      <div class="flex flex-col gap-1">
        <p class="text-xs font-medium tracking-wide text-fg-3 uppercase">{t('chat.crash.lastOutput')}</p>
        <TerminalBlock text={crash.stderr.join('\n')} />
      </div>
    {/if}
  </div>
</section>
