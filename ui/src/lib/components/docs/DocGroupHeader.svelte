<script lang="ts">
  import Plus from '@lucide/svelte/icons/plus';
  import type { Snippet } from 'svelte';

  import type { IconComponent } from '../icon';
  import IconButton from '../IconButton.svelte';

  interface Props {
    /** How many columns the row spans. */
    span: number;
    icon?: IconComponent;
    label: string;
    /** The project's name, set in the mono face after the label. */
    name?: string | null;
    /** "2 of 64". */
    count: string;
    /** The label keeps its width and the name gives way, which is how a scope reads. */
    fixedLabel?: boolean;
    /** Something quiet after the name, e.g. a badge. */
    badge?: Snippet;
    /** The plus that adds a document to this group. */
    add?: { label: string; disabled: boolean; onadd: () => void };
  }

  let { span, icon: Icon, label, name = null, count, fixedLabel = false, badge, add }: Props = $props();
</script>

<tr>
  <th colspan={span} scope="colgroup" class="h-11 border-b border-line/60 bg-surface-2/55 py-0 pr-2 pl-3 text-left text-[11px] leading-[16px] font-medium tracking-wider text-fg-3 uppercase">
    <div class="flex items-center gap-2">
      <span class="flex min-w-0 flex-1 items-center gap-1.5">
        {#if Icon}<Icon size={14} aria-hidden="true" class="shrink-0" />{/if}
        <!-- Label, project name and count share one baseline, whatever their font. -->
        <span class="flex min-w-0 items-baseline gap-1.5">
          <span class={fixedLabel ? 'shrink-0' : 'truncate'}>{label}</span>
          {#if name}<span class="min-w-0 truncate font-mono text-xs tracking-normal normal-case">{name}</span>{/if}
          <span class="tabular ml-1.5 shrink-0 text-xs tracking-wide normal-case">{count}</span>
        </span>
        {@render badge?.()}
      </span>

      {#if add}
        <IconButton icon={Plus} label={add.label} disabled={add.disabled} onclick={add.onadd} />
      {/if}
    </div>
  </th>
</tr>
