<script lang="ts">
  import Info from '@lucide/svelte/icons/info';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';

  import type { BarAction } from './action-model';
  import { actionRunner } from './action-runner.svelte';

  interface Props {
    actions: BarAction[];
  }

  let { actions }: Props = $props();
</script>

<!-- Each action is a row like a settings row: what it does on the left, its button on the right. The note is a caption. -->
<section class="flex flex-col gap-3" aria-labelledby="actions-title">
  <h3 id="actions-title" class="text-sm font-medium tracking-wider text-fg-3 uppercase">{t('extensions.bar.heading')}</h3>

  <ul class="divide-y divide-line rounded-lg border border-line bg-surface-1">
    {#each actions as action (action.title)}
      <li class="flex items-start justify-between gap-6 px-4 py-3">
        <div class="flex min-w-0 flex-col gap-0.5">
          <p class="text-base font-medium text-fg">{action.title}</p>
          <p class="text-sm text-fg-2">{action.description}</p>
        </div>

        <!-- The button's top edge lines up with the top of the title's capitals (5px into its 21px line), so the gap
             above both is the same. -->
        <div class="shrink-0 pt-[5px]">
          <Button variant={action.danger ? 'dangerQuiet' : 'secondary'} size="sm" disabled={actionRunner.phase !== 'idle'} onclick={() => void actionRunner.begin(action.request)}>
            {#snippet leading()}<action.icon size={ICON.sm} aria-hidden="true" />{/snippet}
            {action.label}
          </Button>
        </div>
      </li>
    {/each}
  </ul>

  <p class="flex items-center gap-2 text-xs text-fg-3">
    <Info size={ICON.sm} class="shrink-0 text-info" aria-hidden="true" />
    {t('extensions.bar.note')}
  </p>
</section>
