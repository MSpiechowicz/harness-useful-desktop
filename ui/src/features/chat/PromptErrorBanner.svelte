<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';

  import Button from '$lib/components/Button.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { PromptError } from '$lib/chat/types';

  interface Props {
    error: PromptError;
    canRetry: boolean;
    onretry: () => void;
  }

  let { error, canRetry, onretry }: Props = $props();
</script>

<div role="alert" class="chat-col mb-2 flex items-start gap-3">
  <div class="flex flex-1 items-start gap-3 chat-radius border border-danger-edge bg-danger-wash bg-clip-padding px-4 py-3">
    <CircleAlert size={18} class="mt-0.5 shrink-0 text-danger" aria-hidden="true" />
    <div class="min-w-0 flex-1">
      <p class="text-base font-medium text-fg">{t('chat.promptError.title')}</p>
      <p class="text-sm break-words text-fg-2">{error.message}</p>
    </div>
    {#if error.retryable && canRetry}
      <Button onclick={onretry}>{t('chat.promptError.retry')}</Button>
    {/if}
  </div>
</div>
