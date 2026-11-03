<script lang="ts">
  import Download from '@lucide/svelte/icons/download';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import { nav } from '$lib/stores/nav.svelte';

  import FamilyInstall from './FamilyInstall.svelte';
  import type { HookMember } from './hook-format';

  interface Props {
    member: HookMember;
    /** Runs once the installer finished well. */
    onfinished?: () => void;
  }

  let { member, onfinished }: Props = $props();
</script>

<!-- A companion installs in place. Harness Skills install per harness, which the Ecosystem page does. -->
{#if member === 'skills'}
  <Button onclick={() => nav.go('ecosystem')}>
    {#snippet leading()}<Download size={ICON.sm} aria-hidden="true" />{/snippet}
    {t('hooks.waiting.openEcosystem')}
  </Button>
{:else}
  <FamilyInstall {member} {onfinished} />
{/if}
