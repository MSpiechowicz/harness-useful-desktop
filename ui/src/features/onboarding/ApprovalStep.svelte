<script lang="ts">
  import ShieldAlert from '@lucide/svelte/icons/shield-alert';

  import { cx } from '$lib/utils/cx';

  import { WIDTH } from './layout';
  import StepHeader from './StepHeader.svelte';
</script>

<div class={cx('mx-auto flex w-full flex-col gap-8', WIDTH.narrow)}>
  <StepHeader
    eyebrow="Good to know"
    title="OMP can run tools without asking"
    body="OMP’s tool approval mode defaults to “yolo”: tools run without asking for confirmation, exactly as they do in your terminal."
  />

  <div class="flex items-start gap-4 rounded-lg border border-line bg-warning-wash p-5">
    <ShieldAlert class="mt-0.5 shrink-0 text-warning" size={20} aria-hidden="true" />
    <ul class="flex list-disc flex-col gap-2.5 pl-5 text-base text-fg marker:text-warning">
      <li>Harness Desktop respects the mode you configured (<code class="font-mono text-sm">tools.approvalMode</code>) and never changes it behind your back.</li>
      <li>The active mode is always shown in the chat header, so you can see at a glance whether tools will ask first.</li>
      <li>You can override the mode for a single session before it starts, for example to require approval.</li>
    </ul>
  </div>
</div>
