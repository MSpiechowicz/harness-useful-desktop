<script lang="ts">
  import Pause from '@lucide/svelte/icons/pause';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import { NOTICE_LOOKS } from '$lib/components/notice';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';

  import type { Companion } from '../onboarding/setup-model';
  import FamilyInstall from './FamilyInstall.svelte';
  import { MEMBER_LABELS } from './hook-format';

  interface Props {
    member: Companion;
    /** Installed but switched off, which Settings fixes. Otherwise it is not installed. */
    off: boolean;
    /** How many hooks wait for it. */
    count: number;
  }

  let { member, off, count }: Props = $props();

  const look = NOTICE_LOOKS.info;
  const name = $derived(t(MEMBER_LABELS[member]));

  function openSettings(): void {
    viewState.settingsTarget = 'memory';
    nav.go('settings');
  }
</script>

<!-- The hooks that need a Harness family member wait for it: one line above the list says which, and fixes it where it is shown. -->
<div class={`flex shrink-0 items-center gap-3 border-b border-line px-5 py-2.5 ${look.box}`} role="status">
  <Pause class={`shrink-0 ${look.color}`} size={ICON.sm} aria-hidden="true" />

  <div class="flex min-w-0 flex-1 flex-col">
    <p class="text-sm font-medium text-fg">{t(off ? 'hooks.waiting.off.title' : 'hooks.waiting.missing.title', { member: name })}</p>
    <p class="text-sm text-fg-2">{tn(off ? 'hooks.paused.body.off' : 'hooks.paused.body.missing', count, { member: name })}</p>
  </div>

  {#if off}
    <Button onclick={openSettings}>{t('hooks.waiting.openSettings')}</Button>
  {:else}
    <FamilyInstall {member} />
  {/if}
</div>
