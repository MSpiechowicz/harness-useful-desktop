<script lang="ts">
  import { cx } from '$lib/utils/cx';

  import { ICON } from '../icon';
  import { NOTICE_LOOKS } from '../notice';

  interface Props {
    title: string;
    /** Why the file breaks the format. */
    problem: string;
    /** The file's name, set in the mono face. */
    file: string;
  }

  let { title, problem, file }: Props = $props();

  const look = NOTICE_LOOKS.error;
</script>

<div class={cx('flex items-start gap-3 rounded-md p-3', look.box)} role="alert">
  <span class="flex h-5 shrink-0 items-center"><look.icon class={look.color} size={ICON.sm} aria-hidden="true" /></span>
  <div class="flex min-w-0 flex-col gap-0.5">
    <p class="text-sm font-medium text-fg">{title}</p>
    <p class="text-sm break-words text-fg-2">{problem}</p>
    <p class="mt-1 font-mono text-xs break-all text-fg-3">{file}</p>
  </div>
</div>
