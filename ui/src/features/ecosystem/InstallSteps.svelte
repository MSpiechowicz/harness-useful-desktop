<script lang="ts">
  import Terminal from '@lucide/svelte/icons/terminal';

  import CopyButton from '$lib/components/CopyButton.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { EcosystemId } from '$lib/ipc/bindings/EcosystemId';
  import type { InstallStep } from '$lib/ipc/bindings/InstallStep';

  import { productStepTitle } from './product-copy';

  interface Props {
    steps: InstallStep[];
    /** The companion the steps install; its catalog titles are translated. */
    product: EcosystemId;
  }

  let { steps, product }: Props = $props();
</script>

<div class="flex flex-col gap-2.5">
  <ol class="flex flex-col gap-2.5" aria-label={t('ecosystem.install.steps')}>
    {#each steps as step, index (step.command)}
      {@const title = productStepTitle(product, index, step.label)}
      <li class="flex flex-col gap-1">
        <p class="text-sm text-fg-2">{#if steps.length > 1}<span class="tabular text-fg-3">{index + 1}.</span>{' '}{/if}{title}</p>
        <!-- Shown as text only: the command is for the user's terminal, never run by the app. -->
        <div class="flex items-start gap-1 rounded-md border border-line bg-surface-2 py-1 pr-1 pl-3">
          <code class="min-w-0 flex-1 py-1 font-mono text-sm leading-snug [overflow-wrap:anywhere] whitespace-pre-wrap text-fg">{step.command}</code>
          <CopyButton text={step.command} label={t('ecosystem.install.copy', { label: title })} control />
        </div>
      </li>
    {/each}
  </ol>

  <p class="flex items-center gap-1.5 text-xs text-fg-3">
    <Terminal size={16} class="shrink-0" aria-hidden="true" />
    {t('ecosystem.install.note')}
  </p>
</div>
