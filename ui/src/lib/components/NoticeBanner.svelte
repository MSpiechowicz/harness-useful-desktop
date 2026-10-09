<script lang="ts">
  import ChevronDown from '@lucide/svelte/icons/chevron-down';

  import type { Notice } from '$lib/ipc/bindings/Notice';
  import { cx } from '$lib/utils/cx';

  import { ICON } from './icon';
  import { NOTICE_LOOKS } from './notice';

  interface Props {
    notices: Notice[];
  }

  let { notices }: Props = $props();

  let expanded = $state(false);

  // Several calm notes fold into one line so they do not push the content down; anything that needs attention stays open.
  const folded = $derived(notices.length > 1 && notices.every((notice) => notice.level === 'info') && !expanded);
</script>

{#if notices.length > 0}
  {#if folded}
    {@const look = NOTICE_LOOKS.info}
    <div class={cx('flex items-center gap-3 rounded-md border border-line px-3.5 py-2 text-sm text-fg', look.box)} role="status">
      <look.icon class="shrink-0 {look.color}" size={ICON.sm} aria-hidden="true" />
      <span class="min-w-0 flex-1 truncate">{notices[0].message}</span>
      <button type="button" class="inline-flex shrink-0 items-center gap-1 rounded-sm px-1.5 py-0.5 text-fg-2 transition-colors hover:bg-hover hover:text-fg" aria-expanded="false" onclick={() => (expanded = true)}>
        +{notices.length - 1} more
        <ChevronDown size={ICON.sm} aria-hidden="true" />
      </button>
    </div>
  {:else}
    <ul class="flex flex-col gap-2" aria-label="Notices">
      {#each notices as notice, index (index)}
        {@const look = NOTICE_LOOKS[notice.level]}
        <li class="flex items-start gap-3 rounded-md border border-line px-3.5 py-2.5 text-sm text-fg {look.box}" role={notice.level === 'error' ? 'alert' : 'status'}>
          <look.icon class="mt-0.5 shrink-0 {look.color}" size={ICON.sm} aria-hidden="true" />
          <span class="flex-1">{notice.message}</span>
          {#if expanded && index === 0}
            <button type="button" class="shrink-0 rounded-sm px-1.5 text-fg-2 transition-colors hover:bg-hover hover:text-fg" aria-expanded="true" onclick={() => (expanded = false)}>Show less</button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
{/if}
