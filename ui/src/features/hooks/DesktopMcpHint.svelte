<script lang="ts">
  import Info from '@lucide/svelte/icons/info';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';

  import { onboarding } from '../onboarding/onboarding-state.svelte';
  import { hooks } from './hooks-data.svelte';

  interface Props {
    /** Runs before the Recommended setup opens, e.g. to close the dialog this sits in. */
    onleave?: () => void;
  }

  let { onleave }: Props = $props();

  // The machine is read once, when the hint first has a place on screen.
  $effect(() => {
    void hooks.readDesktopMcp();
  });

  function openSetup(): void {
    onleave?.();
    onboarding.restart('setup');
  }
</script>

<!-- Agents write hooks through the app's own MCP server. Without it in any harness there is nobody to write them, so the way to add it is one click away. -->
{#if hooks.desktopMcp === 'missing'}
  <div class="flex flex-wrap items-center justify-between gap-x-4 gap-y-2 rounded-md bg-info-wash px-3.5 py-2.5 text-sm" role="note">
    <p class="flex min-w-0 items-center gap-2 text-fg-2"><Info size={ICON.sm} class="shrink-0 text-info" aria-hidden="true" />{t('hooks.mcpHint')}</p>
    <Button variant="ghost" onclick={openSetup}>{t('hooks.mcpHint.action')}</Button>
  </div>
{/if}
