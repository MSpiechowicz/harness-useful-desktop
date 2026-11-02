<script lang="ts" generics="Row extends { id: number | string }">
  import ThumbsUp from '@lucide/svelte/icons/thumbs-up';
  import type { ComponentProps, Snippet } from 'svelte';

  import { t } from '$lib/i18n/i18n.svelte';
  import { formatCount } from '$lib/utils/format';

  import FamilyCallRow from './FamilyCallRow.svelte';

  interface Props extends Pick<ComponentProps<typeof FamilyCallRow>, 'call' | 'active'> {
    heading: string;
    /** Beside the title: the first title the card knows, else the count said in words. Left out of a failed call. */
    hint: string;
    /** The rows the agent rated, as far as anything is known of each. */
    rows: Row[];
    /** The reason the agent gave, said once for all the rows. */
    reason: string | null;
    /** What the body says when no row was rated. */
    none: string;
    row: Snippet<[Row]>;
  }

  let { call, active, heading, hint, rows, reason, none, row: rowOf }: Props = $props();

  const count = $derived(rows.length);
</script>

<FamilyCallRow
  {call}
  {active}
  icon={ThumbsUp}
  {heading}
  hint={call.status === 'error' ? null : hint}
  done={{ tone: count > 0 ? 'info' : 'idle', label: t('chat.family.feedback.pill', { count: formatCount(count) }) }}
>
  {#snippet content()}
    {#if count === 0}
      <p class="px-[18px] py-3.5 text-sm text-fg-3">{none}</p>
    {:else}
      <section>
        <p class="px-[18px] pt-3 pb-1.5 text-xs text-fg-3">{t('chat.family.search.caption.useful')}</p>
        {#if reason}<p class="line-clamp-2 px-[18px] pb-2 text-[12.5px] leading-snug text-fg-2" title={reason}>{t('chat.family.row.reason', { reason })}</p>{/if}
        <ul class="flex flex-col divide-y divide-line">
          {#each rows as row (row.id)}
            {@render rowOf(row)}
          {/each}
        </ul>
      </section>
    {/if}
  {/snippet}
</FamilyCallRow>
