<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import Copy from '@lucide/svelte/icons/copy';

  import { t } from '$lib/i18n/i18n.svelte';
  import { toasts } from '$lib/stores/toasts.svelte';
  import { cx } from '$lib/utils/cx';

  import IconButton from './IconButton.svelte';

  interface Props {
    /** Text placed on the clipboard. */
    text: string;
    label?: string;
    /** Full control height; without it the button is the compact 28px size the onboarding card still uses. */
    control?: boolean;
    class?: string;
  }

  let { text, label, control = false, class: className }: Props = $props();

  const CONFIRM_MS = 1600;

  let copied = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function copy(): Promise<void> {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      toasts.push({ title: t('components.copy.failed'), description: t('components.copy.unavailable'), tone: 'warning' });

      return;
    }

    copied = true;
    clearTimeout(timer);
    timer = setTimeout(() => (copied = false), CONFIRM_MS);
  }
</script>

<IconButton icon={copied ? Check : Copy} label={copied ? t('components.copy.copied') : (label ?? t('components.copy.label'))} class={cx(!control && 'size-7!', className)} onclick={copy} />
