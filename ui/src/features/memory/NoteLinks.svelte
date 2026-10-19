<script lang="ts">
  import ArrowDownLeft from '@lucide/svelte/icons/arrow-down-left';
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right';

  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { NoteLink } from '$lib/ipc/bindings/NoteLink';

  import NoteKindBadge from './NoteKindBadge.svelte';
  import { spelledOut } from './script-format';

  interface Props {
    links: NoteLink[];
    /** A link was chosen; left out where the links are only shown (the editor). */
    onopen?: (id: number) => void;
    labelledby?: string;
  }

  let { links, onopen, labelledby }: Props = $props();

  const ROW = 'flex h-10 w-full items-center gap-2.5 rounded-md px-1.5 text-left';
</script>

{#snippet row(link: NoteLink)}
  {@const Direction = link.direction === 'out' ? ArrowUpRight : ArrowDownLeft}
  <Direction size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />
  <span class="min-w-0 flex-1 truncate text-sm text-fg">{spelledOut(link.title)}</span>
  <span class="shrink-0 text-xs text-fg-3">{spelledOut(link.relation).replaceAll('_', ' ')}</span>
  <NoteKindBadge kind={link.kind} />
{/snippet}

{#if links.length === 0}
  <p class="text-sm text-fg-3">{t('memory.notes.detail.noLinks')}</p>
{:else}
  <ul class="-mx-1.5 flex flex-col" aria-labelledby={labelledby}>
    {#each links as link (`${link.id}:${link.direction}:${link.relation}`)}
      {@const title = t(link.direction === 'out' ? 'memory.notes.link.out' : 'memory.notes.link.in')}
      <li>
        {#if onopen}
          <button type="button" {title} class="{ROW} transition-colors duration-[var(--dur-fast)] hover:bg-hover active:bg-press" onclick={() => onopen(link.id)}>
            {@render row(link)}
          </button>
        {:else}
          <div {title} class={ROW}>{@render row(link)}</div>
        {/if}
      </li>
    {/each}
  </ul>
{/if}
