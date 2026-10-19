<script lang="ts">
  import Plus from '@lucide/svelte/icons/plus';

  import Button from '../Button.svelte';
  import { ICON } from '../icon';

  interface Props {
    label: string;
    onclick: () => void;
  }

  let { label, onclick }: Props = $props();
</script>

<!-- The primary action of a documents view, in its header and in its empty state. -->
<Button variant="primary" {onclick}>
  {#snippet leading()}<Plus size={ICON.sm} aria-hidden="true" />{/snippet}
  {label}
</Button>
