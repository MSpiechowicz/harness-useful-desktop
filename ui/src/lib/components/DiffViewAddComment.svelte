<script lang="ts">
  import Plus from '@lucide/svelte/icons/plus';

  import { t } from '$lib/i18n/i18n.svelte';
  import type { PickHandlers } from '$lib/git/comment-range';
  import type { CommentSide } from '$lib/ipc/bindings/CommentSide';
  import { cx } from '$lib/utils/cx';

  interface Props {
    /** The line's +/- sign, shown until the row is hovered. */
    sign: string;
    /** Color classes for the sign. */
    tone?: string;
    index: number;
    side: CommentSide;
    pick: PickHandlers;
  }

  let { sign, tone = 'text-fg-3', index, side, pick }: Props = $props();
</script>

<!-- The sign column of a diff line; hovering the row swaps the sign for a comment button, like a code review site. -->
<span class={cx('relative w-4 shrink-0 text-center font-mono text-[14px] leading-[20px] select-none', tone)} aria-hidden="true">
  <span class="group-hover/line:invisible">{sign}</span>
  <button
    type="button"
    tabindex="-1"
    title={t('components.diff.commentLine')}
    class="invisible absolute inset-0 m-auto grid size-4 place-items-center rounded-sm bg-accent-fill text-on-accent group-hover/line:visible"
    onclick={(event) => {
      pick.start(index, side, event.shiftKey);
      pick.finish();
    }}
  >
    <Plus size={12} strokeWidth={2.5} />
  </button>
</span>
