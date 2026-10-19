<script lang="ts">
  import type { Snippet } from 'svelte';

  import { cx } from '$lib/utils/cx';

  import type { IconComponent } from '../icon';
  import { ICON } from '../icon';
  import { NOTICE_LOOKS } from '../notice';

  interface Props {
    icon: IconComponent;
    title: string;
    body: string;
    /** The way to fix what the strip says: a button. */
    action: Snippet;
  }

  let { icon: Icon, title, body, action }: Props = $props();

  const look = NOTICE_LOOKS.info;
</script>

<!-- One line above a list that says what holds it back, and fixes it where it is shown. -->
<div class={cx('flex shrink-0 items-center gap-3 border-b border-line px-5 py-2.5', look.box)} role="status">
  <Icon class={cx('shrink-0', look.color)} size={ICON.sm} aria-hidden="true" />

  <div class="flex min-w-0 flex-1 flex-col">
    <p class="text-sm font-medium text-fg">{title}</p>
    <p class="text-sm text-fg-2">{body}</p>
  </div>

  {@render action()}
</div>
