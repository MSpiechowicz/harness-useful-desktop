<script lang="ts">
  import ShieldAlert from '@lucide/svelte/icons/shield-alert';

  import { MODE_ORDER, MODES } from '../chat/approval-modes';
  import { cx } from '$lib/utils/cx';

  import { WIDTH } from './layout';
  import StepHeader from './StepHeader.svelte';

  /** How each OMP mode behaves in practice, next to its name. */
  const DETAILS: Record<(typeof MODE_ORDER)[number], { cli: string; when: string; goodFor: string }> = {
    alwaysAsk: { cli: 'always-ask', when: 'Every tool call waits for you. Safest, slowest.', goodFor: 'unfamiliar repositories and sensitive machines' },
    write: { cli: 'write', when: 'Reading is free; editing files or running commands waits for you.', goodFor: 'day-to-day work where you want a say before changes' },
    yolo: { cli: 'yolo', when: 'Nothing waits. Fastest, and exactly how OMP runs in your terminal.', goodFor: 'trusted repositories, sandboxes and quick iteration' },
  };

  const TONE: Record<string, string> = {
    success: 'border-success/30',
    info: 'border-accent/30',
    warning: 'border-warning/40 bg-warning-wash',
  };
</script>

<div class={cx('mx-auto flex w-full flex-col gap-8 [@media(max-height:720px)]:gap-4', WIDTH.wide)}>
  <StepHeader
    eyebrow="Good to know"
    title="OMP can run tools without asking"
    body="OMP’s tool approval mode defaults to “yolo”: tools run without asking for confirmation, exactly as in your terminal."
  />

  <ul class="grid grid-cols-3 gap-4" aria-label="OMP approval modes">
    {#each MODE_ORDER as mode (mode)}
      {@const info = MODES[mode]}
      <li class={cx('flex flex-col gap-2 rounded-lg border bg-glass p-5', TONE[info.tone])}>
        <div class="flex items-center justify-between gap-2">
          <p class="text-base font-medium text-fg first-letter:uppercase">{info.label}</p>
          {#if mode === 'yolo'}<span class="rounded-full bg-warning/15 px-2 py-0.5 text-xs font-medium text-warning">OMP default</span>{/if}
        </div>
        <code class="font-mono text-xs text-fg-3">--approval-mode {DETAILS[mode].cli}</code>
        <p class="text-sm text-fg-2">{DETAILS[mode].when}</p>
        <p class="mt-auto border-t border-line pt-2 text-xs text-fg-3">Good for {DETAILS[mode].goodFor}.</p>
      </li>
    {/each}
  </ul>

  <div class="flex items-start gap-4 rounded-lg border border-line bg-glass p-5">
    <ShieldAlert class="mt-0.5 shrink-0 text-warning" size={20} aria-hidden="true" />
    <ul class="flex list-disc flex-col gap-2 pl-5 text-sm text-fg-2 marker:text-warning">
      <li>Harness Desktop respects the mode you configured (<code class="font-mono text-xs">tools.approvalMode</code>) and never changes it behind your back.</li>
      <li>The active mode is always shown in the chat header, so you can see at a glance whether tools will ask first.</li>
      <li>You can pick a stricter mode for a single chat before it starts, without touching your OMP settings.</li>
    </ul>
  </div>
</div>
