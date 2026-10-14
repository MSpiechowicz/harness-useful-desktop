<script lang="ts">
  import RotateCw from '@lucide/svelte/icons/rotate-cw';

  import Button from './Button.svelte';
  import { ICON } from './icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  interface Props {
    /** What gets refreshed, spoken and shown as the tooltip-free name: "Refresh sessions". */
    label: string;
    disabled?: boolean;
    /** A refresh is in flight: the arrows spin and the button cannot be pressed again. */
    busy?: boolean;
    onclick: () => void;
  }

  let { label, disabled = false, busy = false, onclick }: Props = $props();
</script>

<!-- Every refresh in the app is this same bordered button, so it reads like its neighbours (Search, New chat)
     rather than a stray icon. -->
<Button variant="secondary" aria-label={label} disabled={disabled || busy} {onclick}>
  {#snippet leading()}<RotateCw size={ICON.sm} class={cx(busy && 'animate-spin')} aria-hidden="true" />{/snippet}
  <span class="max-[1100px]:sr-only">{t('components.refresh')}</span>
</Button>
