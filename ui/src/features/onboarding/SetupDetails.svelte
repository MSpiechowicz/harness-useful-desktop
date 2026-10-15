<script lang="ts">
  import type { Snippet } from 'svelte';

  import Disclosure from '$lib/components/Disclosure.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  interface Props {
    /** Closed to start with, so a card stays compact. */
    open?: boolean;
    /** Quiet text after the label, such as a count. */
    meta?: string;
    children: Snippet;
  }

  let { open = $bindable(false), meta, children }: Props = $props();
</script>

<!-- The toggle is only as wide as its words, so its keyboard ring hugs them; what it opens sits straight on the card, no box of its own. -->
<Disclosure class="border-t border-line pt-1 [&>button]:min-h-control [&>button]:w-fit [&>button]:pr-2" label={t('onboarding.setup.details')} {meta} bind:open>
  <div class="flex flex-col gap-5 pt-1 pb-1 text-sm text-fg-2">{@render children()}</div>
</Disclosure>
