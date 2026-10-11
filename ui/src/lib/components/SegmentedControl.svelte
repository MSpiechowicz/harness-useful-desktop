<script lang="ts" generics="T extends string">
  import { uid } from '$lib/utils/ids';

  import OptionStrip from './OptionStrip.svelte';
  import type { StripOption } from './strip';

  interface Props {
    options: StripOption<T>[];
    value: T;
    label: string;
    onchange?: (value: T) => void;
    /** Matches the button size beside it: `sm` in toolbars, `md` in forms and settings rows. */
    size?: 'sm' | 'md';
  }

  let { options, value = $bindable(), label, onchange, size = 'sm' }: Props = $props();
</script>

<OptionStrip {options} bind:value kind="radio" appearance="pill" {label} {size} idBase={uid('segmented')} {onchange} />
