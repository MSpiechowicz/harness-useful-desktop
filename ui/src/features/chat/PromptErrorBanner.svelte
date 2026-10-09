<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';

  import Button from '$lib/components/Button.svelte';
  import type { PromptError } from '$lib/chat/types';

  interface Props {
    error: PromptError;
    canRetry: boolean;
    onretry: () => void;
  }

  let { error, canRetry, onretry }: Props = $props();
</script>

<div role="alert" class="mx-auto mb-2 flex w-full max-w-3xl items-start gap-3 px-6">
  <div class="flex flex-1 items-start gap-3 rounded-lg border border-danger/40 bg-danger-wash px-4 py-3">
    <CircleAlert size={18} class="mt-0.5 shrink-0 text-danger" aria-hidden="true" />
    <div class="min-w-0 flex-1">
      <p class="text-base font-medium text-fg">The agent could not finish that</p>
      <p class="text-sm break-words text-fg-2">{error.message}</p>
    </div>
    {#if error.retryable && canRetry}
      <Button size="sm" onclick={onretry}>Retry</Button>
    {/if}
  </div>
</div>
