<script lang="ts">
  import { ICON } from '$lib/components/icon';
  import { NOTICE_LOOKS } from '$lib/components/notice';

  interface Props {
    /** Whether the banner shows at all. */
    offline: boolean;
    /** What the list on screen is while the team database cannot be reached. */
    message: string;
    /** A second line under it, for what else follows. */
    detail?: string;
  }

  let { offline, message, detail }: Props = $props();

  const look = NOTICE_LOOKS.warning;
</script>

<!-- While the team database cannot be reached the list is not what is kept there. This says so, so it is not taken for it. -->
{#if offline}
  <div class="flex shrink-0 items-center gap-3 border-b border-line px-4 py-2 {look.box}" role="status">
    <span class="flex h-5 shrink-0 items-center"><look.icon class={look.color} size={ICON.sm} aria-hidden="true" /></span>
    <div class="min-w-0 flex-1">
      <p class="text-sm text-fg">{message}</p>
      {#if detail}<p class="text-xs text-fg-3">{detail}</p>{/if}
    </div>
  </div>
{/if}
