<script lang="ts" generics="T extends string">
  import type { Snippet } from 'svelte';

  import { rise } from '$lib/design/motion';
  import { cx } from '$lib/utils/cx';
  import { uid } from '$lib/utils/ids';

  import OptionStrip from './OptionStrip.svelte';
  import type { StripOption } from './strip';

  interface Props {
    items: StripOption<T>[];
    value: T;
    label: string;
    onchange?: (value: T) => void;
    /** Layout classes of the wrapper; replaces the default `gap-4` (pass `min-h-0 flex-1` to fill a parent). */
    class?: string;
    /** Extra classes for the tab strip, e.g. horizontal padding. */
    stripClass?: string;
    /** Extra classes for the panel. */
    panelClass?: string;
    /** Renders the active panel's content. */
    panel: Snippet<[T]>;
  }

  let { items, value = $bindable(), label, onchange, class: className = 'gap-4', stripClass, panelClass, panel }: Props = $props();

  const idBase = uid('tabs');
</script>

<div class={cx('flex flex-col', className)}>
  <OptionStrip options={items} bind:value kind="tabs" appearance="underline" {label} {idBase} {onchange} class={stripClass} />

  {#key value}
    <div id="{idBase}-panel" role="tabpanel" aria-labelledby="{idBase}-tab-{value}" tabindex="0" in:rise={{ y: 6 }} class={panelClass}>
      {@render panel(value)}
    </div>
  {/key}
</div>
