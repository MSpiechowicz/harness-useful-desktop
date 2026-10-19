<script lang="ts">
  import { slide } from 'svelte/transition';

  import { scaled } from '$lib/design/motion';
  import { ICON } from '$lib/components/icon';
  import { NOTICE_LOOKS } from '$lib/components/notice';
  import { cx } from '$lib/utils/cx';

  import MemoryInstall from '../memory/MemoryInstall.svelte';

  interface Props {
    /** What the installed Harness Memory cannot do yet. */
    text: string;
    /** Looks at the installed Harness Memory again once the update ran. */
    onfinished: () => Promise<void>;
    class?: string;
    id?: string;
  }

  let { text, onfinished, class: className, id }: Props = $props();

  const look = NOTICE_LOOKS.warning;
</script>

<!-- An older Harness Memory is updated right here, with the installer the Notes and Scripts views use, in the look every notice shares. -->
<div {id} transition:slide={{ duration: scaled(200) }} class={cx('flex items-center gap-3 rounded-md border border-line px-4 py-2.5 text-sm leading-5 text-fg', look.box, className)} role="status">
  <look.icon class={cx('shrink-0', look.color)} size={ICON.sm} aria-hidden="true" />
  <span class="min-w-0 flex-1">{text}</span>
  <MemoryInstall update inline {onfinished} />
</div>
