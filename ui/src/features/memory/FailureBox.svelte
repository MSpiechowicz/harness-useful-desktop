<script lang="ts">
  import { ICON } from '$lib/components/icon';
  import { NOTICE_LOOKS } from '$lib/components/notice';

  interface Props {
    /** Why it failed, as the server worded it. */
    message: string;
  }

  let { message }: Props = $props();

  const look = NOTICE_LOOKS.error;
</script>

<div class="flex items-start gap-3 rounded-md p-3 {look.box}" role="alert">
  <span class="flex h-5 shrink-0 items-center"><look.icon class={look.color} size={ICON.sm} aria-hidden="true" /></span>
  <span class="min-w-0 text-sm text-fg [overflow-wrap:anywhere]">{message}</span>
</div>
