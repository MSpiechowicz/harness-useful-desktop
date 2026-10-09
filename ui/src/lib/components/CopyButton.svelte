<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import Copy from '@lucide/svelte/icons/copy';

  import { toasts } from '$lib/stores/toasts.svelte';

  import IconButton from './IconButton.svelte';

  interface Props {
    /** Text placed on the clipboard. */
    text: string;
    label?: string;
  }

  let { text, label = 'Copy' }: Props = $props();

  const CONFIRM_MS = 1600;

  let copied = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function copy(): Promise<void> {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      toasts.push({ title: 'Could not copy', description: 'The clipboard is not available. Select the text and copy it manually.', tone: 'warning' });

      return;
    }

    copied = true;
    clearTimeout(timer);
    timer = setTimeout(() => (copied = false), CONFIRM_MS);
  }
</script>

<IconButton icon={copied ? Check : Copy} label={copied ? 'Copied' : label} size="sm" onclick={copy} />
