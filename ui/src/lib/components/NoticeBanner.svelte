<script lang="ts">
  import type { Notice } from '$lib/ipc/bindings/Notice';

  import { NOTICE_LOOKS } from './notice';

  interface Props {
    notices: Notice[];
  }

  let { notices }: Props = $props();
</script>

{#if notices.length > 0}
  <ul class="flex flex-col gap-2" aria-label="Notices">
    {#each notices as notice, index (index)}
      {@const look = NOTICE_LOOKS[notice.level]}
      <li class="flex items-start gap-3 rounded-md border border-line px-3.5 py-2.5 text-sm text-fg {look.box}" role={notice.level === 'error' ? 'alert' : 'status'}>
        <look.icon class="mt-0.5 shrink-0 {look.color}" size={16} aria-hidden="true" />
        <span>{notice.message}</span>
      </li>
    {/each}
  </ul>
{/if}
