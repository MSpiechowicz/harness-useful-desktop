<script lang="ts">
  import Pause from '@lucide/svelte/icons/pause';

  import Button from '$lib/components/Button.svelte';
  import DocNoticeStrip from '$lib/components/docs/DocNoticeStrip.svelte';
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

  const name = $derived(t(MEMBER_LABELS[member]));

  function openSettings(): void {
    viewState.settingsTarget = 'memory';
    nav.go('settings');
  }
</script>

<!-- The hooks that need a Harness family member wait for it: one line above the list says which, and fixes it where it is shown. -->
<DocNoticeStrip icon={Pause} title={t(off ? 'hooks.waiting.off.title' : 'hooks.waiting.missing.title', { member: name })} body={tn(off ? 'hooks.paused.body.off' : 'hooks.paused.body.missing', count, { member: name })}>
  {#snippet action()}
    {#if off}
      <Button onclick={openSettings}>{t('hooks.waiting.openSettings')}</Button>
    {:else}
      <FamilyInstall {member} />
    {/if}
  {/snippet}
</DocNoticeStrip>
