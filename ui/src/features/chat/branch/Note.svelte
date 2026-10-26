<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import type { Snippet } from 'svelte';

  import type { IconComponent } from '$lib/components/icon';
  import { noteParts } from '$lib/git/sync-notes';
  import { cx } from '$lib/utils/cx';

  interface Props {
    tone: 'success' | 'warning' | 'danger';
    icon: IconComponent;
    /** What happened or what is the case, in one line or two. */
    title: string;
    /** Branch names inside `title`: they are set in the code font and the tone colour. */
    names?: string[];
    /** A dim line under the title. */
    detail?: string;
    /** A button at the end, e.g. "Apply stash". */
    action?: Snippet;
  }

  let { tone, icon: Icon, title, names, detail, action }: Props = $props();

  const TONES = {
    success: { box: 'border-success/30 bg-success-wash', badge: 'bg-success ring-success/15', name: 'text-success' },
    warning: { box: 'border-warning/30 bg-warning-wash', badge: 'bg-warning ring-warning/15', name: 'text-warning' },
    danger: { box: 'border-danger/30 bg-danger-wash', badge: 'bg-danger ring-danger/15', name: 'text-danger' },
  } as const;

  const look = $derived(TONES[tone]);
  const parts = $derived(noteParts(title, names));
</script>

<div role="status" class={cx('note mx-2.5 mb-2.5 flex items-center gap-3 rounded-md border px-3 py-2.5', look.box)}>
  <span class={cx('badge grid size-7 shrink-0 place-items-center rounded-full text-canvas ring-4', look.badge, Icon === Check && 'draw')}>
    <Icon size={16} strokeWidth={3} aria-hidden="true" />
  </span>

  <div class="min-w-0 flex-1">
    <div class="text-sm leading-4.5 font-semibold text-fg">
      {#each parts as part, index (index)}{#if part.name}<span class={cx('font-mono', look.name)}>{part.text}</span>{:else}{part.text}{/if}{/each}
    </div>
    {#if detail}<div class="text-xs leading-4 text-fg-3">{detail}</div>{/if}
  </div>

  {#if action}<div class="shrink-0">{@render action()}</div>{/if}
</div>

<style>
  /* The note slides in, its badge pops and a check draws itself. Durations come from the motion tokens, so reduced
     motion shortens them and "off" skips them. */
  .note {
    animation: note-in var(--dur-enter) var(--ease-out-expo) both;
  }

  .badge {
    animation: badge-pop var(--dur-enter) calc(var(--dur-enter) * 0.25) var(--ease-spring) both;
  }

  .draw :global(path) {
    stroke-dasharray: 24;
    stroke-dashoffset: 24;
    animation: check-draw var(--dur-slow) calc(var(--dur-enter) * 0.65) ease-out forwards;
  }

  @keyframes note-in {
    from {
      opacity: 0;
      transform: translateY(calc(var(--shift) * -0.75)) scale(0.98);
    }

    to {
      opacity: 1;
      transform: none;
    }
  }

  @keyframes badge-pop {
    from {
      transform: scale(0);
    }

    to {
      transform: scale(1);
    }
  }

  @keyframes check-draw {
    to {
      stroke-dashoffset: 0;
    }
  }
</style>
