<script lang="ts">
  import Info from '@lucide/svelte/icons/info';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import { motion } from '$lib/stores/motion.svelte';
  import { cx } from '$lib/utils/cx';

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
      <li class="flex items-center justify-between gap-6 px-4 py-3">
        <div class="flex min-w-0 flex-col gap-0.5">
          <p class="text-base font-medium text-fg">{action.title}</p>
          <p class="text-sm text-fg-2">{action.description}</p>
        </div>

        <!-- Centred on the row, so the space above and below the button is the same whatever the text beside it. Every button has the same width, so a column of them reads as one. -->
        <div class="w-40 shrink-0">
          <Button class="w-full" variant={action.danger ? 'dangerQuiet' : action.state === 'ready' ? 'primary' : 'secondary'} disabled={actionRunner.phase !== 'idle' || action.state === 'idle'} onclick={() => void actionRunner.begin(action.request)}>
            {#snippet leading()}<span class={cx('inline-flex', action.state === 'ready' && motion.level === 'full' && 'update-arrow')}><action.icon size={ICON.sm} aria-hidden="true" /></span>{/snippet}
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
