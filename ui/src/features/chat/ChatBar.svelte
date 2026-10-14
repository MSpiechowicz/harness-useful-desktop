<script lang="ts">
  import MessageSquarePlus from '@lucide/svelte/icons/message-square-plus';
  import { fade } from 'svelte/transition';

  import { portal, SHELL_HEADER_SLOT } from '$lib/actions/portal';
  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import Kbd from '$lib/components/Kbd.svelte';
  import { scaled } from '$lib/design/motion';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import { newChat } from '../shell/actions';

  // The open chat names the page; with the rail collapsed it is the only place its name shows.
  const heading = $derived(agentSessions.activeKey ? agentSessions.title(agentSessions.activeKey) : t('chat.title'));
  const ready = $derived(Boolean(harness.active?.detection.installed && project.active));
</script>

<!-- Chat's header, rendered into the shell's top bar like every view's. The open chats live in the rail, under Chat. -->
<header
  data-view-header
  use:portal={SHELL_HEADER_SLOT}
  in:fade|global={{ duration: scaled(160), delay: 80 }}
  out:fade|global={{ duration: scaled(100) }}
  class="flex h-[52px] min-w-0 items-center gap-3 px-2"
>
  <h1 class="min-w-0 flex-1 truncate text-base leading-5 font-medium text-fg">{heading}</h1>

  <div class="flex shrink-0 items-center gap-2">
    <Button variant="primary" size="sm" disabled={!ready} onclick={() => void newChat()}>
      {#snippet leading()}<MessageSquarePlus size={ICON.sm} aria-hidden="true" />{/snippet}
      <span class="max-[1100px]:sr-only">{t('chat.bar.newChat')}</span>
      <span class="max-[1100px]:hidden"><Kbd keys="mod+n" /></span>
    </Button>
  </div>
</header>
