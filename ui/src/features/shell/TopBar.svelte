<script lang="ts">
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import ArrowRight from '@lucide/svelte/icons/arrow-right';

  import { SHELL_HEADER_SLOT } from '$lib/actions/portal';
  import IconButton from '$lib/components/IconButton.svelte';
  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import { nav } from '$lib/stores/nav.svelte';

</script>

<!-- The one bar above the content: history, then the current view's own header (title, state, search, actions).
     Who and where you are (brand, harness, project) live on the rail. -->
<div class="intro-enter intro-top flex h-[52px] shrink-0 items-center gap-2 border-b border-line bg-surface-1/70 px-2" style:--intro-delay="60ms">
  <div class="flex shrink-0 items-center">
    <IconButton icon={ArrowLeft} label={t('common.back')} shortcut="alt+arrowleft" size="sm" side="bottom" disabled={!nav.canGoBack} onclick={() => nav.back()} />
    <IconButton icon={ArrowRight} label={t('shell.topbar.forward')} shortcut="alt+arrowright" size="sm" side="bottom" disabled={!nav.canGoForward} onclick={() => nav.forward()} />
  </div>

  <!-- Stacked cell: during a page switch the outgoing and incoming headers crossfade in place. -->
  <div id={SHELL_HEADER_SLOT} class="grid h-full min-w-0 flex-1 grid-cols-[minmax(0,1fr)] *:col-start-1 *:row-start-1"></div>
</div>
