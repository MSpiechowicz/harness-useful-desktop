<script lang="ts">
  import { cx } from '$lib/utils/cx';

  import type { Rect } from './kernel-layout';

  interface Props {
    rect: Rect;
    /** The stage width: a long line in another language is cut here rather than hanging out of the card. */
    max: number;
    text: string;
    /** A module is hovered or focused: the line is about it and reads brighter. */
    focused: boolean;
  }

  let { rect, max, text, focused }: Props = $props();
</script>

<div
  class={cx('status tabular', focused && 'focused')}
  style:left="{rect.x + rect.w / 2}px"
  style:top="{rect.y + rect.h / 2}px"
  style:max-width="{max}px"
  style:height="{rect.h}px"
  style:line-height="{rect.h}px"
>
  {text}
</div>

<style>
  .status {
    position: absolute;
    transform: translate(-50%, -50%);
    overflow: hidden;
    color: var(--text-2);
    font-size: var(--text-xs);
    text-overflow: ellipsis;
    white-space: nowrap;
    transition: color var(--dur-base);
  }

  .status.focused {
    color: var(--text);
  }
</style>
