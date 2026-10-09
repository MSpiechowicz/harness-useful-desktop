<script lang="ts">
  import Terminal from '@lucide/svelte/icons/terminal';

  import CopyButton from '$lib/components/CopyButton.svelte';
  import type { InstallStep } from '$lib/ipc/bindings/InstallStep';

  interface Props {
    steps: InstallStep[];
  }

  let { steps }: Props = $props();
</script>

<div class="flex flex-col gap-2.5">
  <ol class="flex flex-col gap-2.5" aria-label="Install steps">
    {#each steps as step, index (step.command)}
      <li class="flex flex-col gap-1">
        <p class="text-sm text-fg-2"><span class="tabular text-fg-3">{index + 1}.</span> {step.label}</p>
        <!-- Shown as text only: the command is for the user's terminal, never run by the app. -->
        <div class="flex items-start gap-1 rounded-md border border-line bg-surface-2 py-1 pr-1 pl-3">
          <code class="min-w-0 flex-1 py-1 font-mono text-sm leading-snug [overflow-wrap:anywhere] whitespace-pre-wrap text-fg">{step.command}</code>
          <CopyButton text={step.command} label="Copy: {step.label}" />
        </div>
      </li>
    {/each}
  </ol>

  <p class="flex items-center gap-1.5 text-xs text-fg-3">
    <Terminal size={16} class="shrink-0" aria-hidden="true" />
    Copy these into a terminal yourself. Harness Desktop never runs install commands for you.
  </p>
</div>
