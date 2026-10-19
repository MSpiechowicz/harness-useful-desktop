<script lang="ts">
  import type { NoticeLevel } from '$lib/ipc/bindings/NoticeLevel';

  import { ICON, type IconComponent } from './icon';
  import { NOTICE_LOOKS } from './notice';

  interface Props {
    level: NoticeLevel;
    /** Left out for a notice that is one sentence. */
    title?: string;
    body: string;
    /** In place of the level's own icon. */
    icon?: IconComponent;
  }

  let { level, title, body, icon }: Props = $props();

  const look = $derived(NOTICE_LOOKS[level]);
  const Icon = $derived(icon ?? look.icon);
</script>

<!-- The icon and the text of a notice, inside the box its caller draws. -->
<div class="flex min-w-0 items-start gap-3">
  <span class="flex h-5 shrink-0 items-center"><Icon class={look.color} size={ICON.sm} aria-hidden="true" /></span>
  <div class="flex min-w-0 flex-col gap-0.5">
    {#if title}
      <p class="text-sm font-medium text-fg">{title}</p>
    {/if}
    <p class="text-sm text-fg-2">{body}</p>
  </div>
</div>
