<script lang="ts">
  import type { Snippet } from 'svelte';

  import Badge from '$lib/components/Badge.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { SyncItemKind } from '$lib/ipc/bindings/SyncItemKind';
  import { cx } from '$lib/utils/cx';

  import { KIND_KEYS, type Chip } from './sync-model';

  interface Props {
    kind: SyncItemKind;
    label: string;
    /** A line of small print under the label. */
    sub?: string | null;
    /** The state at the end of the row. */
    chip?: Chip;
    /** The tick box. A list without boxes passes `reserve` when it sits under one with boxes, so both line up. */
    check?: Snippet;
    reserve?: boolean;
    dim?: boolean;
    /** Everything the row has to say beyond its first line. */
    children?: Snippet;
  }

  let { kind, label, sub, chip, check, reserve = false, dim = false, children }: Props = $props();

  const hasBox = $derived(!!check || reserve);
</script>

<!-- One structure for every row of every list: [box] [kind] [name] [state], then the details under the name. The cells of the first line are centred on it, so the box, the badge, the name and the chip share one centre line however tall the name block is; the details run under it. -->
<li class={cx('grid items-center gap-x-3 border-t border-line py-2', hasBox ? 'grid-cols-[var(--control-h)_7.5rem_minmax(0,1fr)_auto]' : 'grid-cols-[7.5rem_minmax(0,1fr)_auto]')}>
  {#if hasBox}
    <span class="flex h-control items-center">{@render check?.()}</span>
  {/if}

  <span class={cx('flex h-control items-center', dim && 'opacity-60')}>
    <Badge size="lg" class="w-full justify-center"><span class="truncate">{t(KIND_KEYS[kind])}</span></Badge>
  </span>

  <div class={cx('flex min-h-control min-w-0 flex-col justify-center', dim && 'opacity-60')}>
    <p class="truncate text-base font-medium text-fg" title={label}>{label}</p>
    {#if sub}
      <p class="text-xs leading-4 text-fg-3 [overflow-wrap:anywhere]">{sub}</p>
    {/if}
  </div>

  <span class="flex h-control items-center justify-end">
    {#if chip}
      <Badge size="lg" tone={chip.tone}>
        {#if chip.busy}
          <Spinner size={12} />
        {/if}
        {chip.label}
      </Badge>
    {/if}
  </span>

  <!-- Whatever else the row has to say runs under the name and the state, so a command is as wide as the room allows. -->
  {#if children}
    <div class={cx('col-span-2 mt-1.5 self-start not-has-[*]:hidden flex min-w-0 flex-col gap-2 pb-1', hasBox ? 'col-start-3' : 'col-start-2', dim && 'opacity-60')}>
      {@render children()}
    </div>
  {/if}
</li>
