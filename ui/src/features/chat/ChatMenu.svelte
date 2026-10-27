<script lang="ts">
  import Activity from '@lucide/svelte/icons/activity';
  import Eye from '@lucide/svelte/icons/eye';
  import EyeOff from '@lucide/svelte/icons/eye-off';

  import MenuItem from '$lib/components/MenuItem.svelte';
  import MoreMenu from '$lib/components/MoreMenu.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';

  import StatsDialog from './StatsDialog.svelte';

  interface Props {
    /** The open chat; without one there are no statistics to show. */
    session?: ChatSession;
  }

  let { session }: Props = $props();

  const toggles = $derived(
    [
      { pref: 'showThinking', label: t('chat.thinking.toggle'), aria: t('chat.thinking.toggleAria'), hide: t('chat.thinking.hide'), show: t('chat.thinking.show') },
      { pref: 'showMessageUsage', label: t('chat.usage.toggle'), aria: t('chat.usage.toggleAria'), hide: t('chat.usage.hide'), show: t('chat.usage.show') },
    ] as const,
  );
</script>

<!-- What the open chat shows (the thought process, the cost under replies) and what it used. Settings has the same switches. -->
<MoreMenu label={t('chat.menu.label')} panelClass="w-48">
  {#snippet children({ close })}
    {#each toggles as toggle (toggle.pref)}
      {@const shown = prefs.value?.[toggle.pref] ?? true}
      <MenuItem
        role="menuitemcheckbox"
        aria-checked={shown}
        aria-label={toggle.aria}
        title={shown ? toggle.hide : toggle.show}
        icon={shown ? Eye : EyeOff}
        onclick={() => void prefs.update({ [toggle.pref]: !shown })}>{toggle.label}</MenuItem
      >
    {/each}
    {#if session}
      <MenuItem
        icon={Activity}
        onclick={() => {
          close();
          session.ui.stats = true;
        }}>{t('chat.stats.trigger')}</MenuItem
      >
    {/if}
  {/snippet}
</MoreMenu>

{#if session}<StatsDialog {session} />{/if}
