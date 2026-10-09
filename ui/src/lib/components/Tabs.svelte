<script lang="ts" generics="T extends string">
  import type { Snippet } from 'svelte';

  import { rise } from '$lib/design/motion';
  import { uid } from '$lib/utils/ids';

  import OptionStrip from './OptionStrip.svelte';
  import type { StripOption } from './strip';

  interface Props {
    items: StripOption<T>[];
    value: T;
    label: string;
    onchange?: (value: T) => void;
    /** Renders the active panel's content. */
    panel: Snippet<[T]>;
  }

  let { items, value = $bindable(), label, onchange, panel }: Props = $props();

  const idBase = uid('tabs');
</script>

<div class="flex flex-col gap-4">
  <OptionStrip options={items} bind:value kind="tabs" appearance="underline" {label} {idBase} {onchange} />

  {#key value}
    <div id="{idBase}-panel" role="tabpanel" aria-labelledby="{idBase}-tab-{value}" tabindex="0" in:rise={{ y: 6 }}>
      {@render panel(value)}
    </div>
  {/key}
</div>
