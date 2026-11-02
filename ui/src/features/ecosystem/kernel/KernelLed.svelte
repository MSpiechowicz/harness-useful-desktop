<script lang="ts">
  import { cx } from '$lib/utils/cx';

  interface Props {
    /** `member` lights in the module's colour, `update` in the accent, `empty` is a hollow socket LED. */
    tone: 'member' | 'update' | 'empty';
    /** A ring spreads from the LED now and then. */
    pulse?: boolean;
  }

  let { tone, pulse = false }: Props = $props();
</script>

<span class={cx('led', tone, pulse && 'pulse')} aria-hidden="true"></span>

<style>
  .led {
    position: relative;
    display: block;
    width: var(--kernel-led);
    height: var(--kernel-led);
    border-radius: 50%;
    background: var(--mc);
  }

  .update {
    background: var(--accent-2);
  }

  .empty {
    background: transparent;
    box-shadow: inset 0 0 0 1px var(--text-3);
  }

  .pulse::after {
    content: '';
    position: absolute;
    inset: calc(var(--kernel-ring-gap) * -1);
    border-radius: 50%;
    border: var(--kernel-ring-w) solid var(--accent-2);
    animation: ring 2s var(--ease-out-expo) infinite;
  }

  @keyframes ring {
    0% {
      transform: scale(1);
      opacity: 0.7;
    }
    100% {
      transform: scale(1.55);
      opacity: 0;
    }
  }
</style>
