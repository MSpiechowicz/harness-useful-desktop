<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import Folder from '@lucide/svelte/icons/folder';

  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import Kbd from '$lib/components/Kbd.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { cx } from '$lib/utils/cx';

  import { WIDTH } from './layout';
  import { onboarding } from './onboarding-state.svelte';
  import StepHeader from './StepHeader.svelte';

  const driver = $derived(onboarding.driver ? harness.get(onboarding.driver) : undefined);

  /** The first things worth knowing once the app opens. */
  const TIPS = $derived<{ keys: string; title: string; body: string }[]>([
    { keys: 'mod+k', title: t('onboarding.done.tip.palette.title'), body: t('onboarding.done.tip.palette.body') },
    { keys: 'mod+n', title: t('onboarding.done.tip.newChat.title'), body: t('onboarding.done.tip.newChat.body') },
    { keys: 'mod+3', title: t('onboarding.done.tip.changes.title'), body: t('onboarding.done.tip.changes.body') },
    { keys: 'shift+?', title: t('onboarding.done.tip.shortcuts.title'), body: t('onboarding.done.tip.shortcuts.body') },
  ]);
</script>

<div class={cx('mx-auto flex w-full flex-col gap-8 [@media(max-height:720px)]:gap-4', WIDTH.wide)}>
  <StepHeader eyebrow={t('onboarding.done.eyebrow')} title={t('onboarding.done.title')} body={t('onboarding.done.body')} />

  <div class="grid grid-cols-2 items-start gap-4">
    <section class="flex flex-col gap-4 rounded-lg border border-line bg-glass p-5" aria-labelledby="setup-summary">
      <h3 id="setup-summary" class="text-sm font-medium tracking-wider text-fg-3 uppercase">{t('onboarding.done.setup')}</h3>
      <dl class="grid grid-cols-[max-content_1fr] items-center gap-x-6 gap-y-4">
        <dt class="text-sm text-fg-3">{t('onboarding.done.driver')}</dt>
        <dd class="flex items-center gap-3 text-base text-fg">
          {#if driver}
            <HarnessBadge info={driver.info} size="sm" />
            <span>{driver.info.name}</span>
            {#if driver.detection.version}<span class="tabular text-sm text-fg-3">v{driver.detection.version}</span>{/if}
          {:else}
            <span class="text-fg-2">{t('onboarding.done.noDriver')}</span>
          {/if}
        </dd>

        <dt class="text-sm text-fg-3">{t('onboarding.done.project')}</dt>
        <dd class="flex items-center gap-3 text-base text-fg">
          {#if project.active}
            <Folder class="text-fg-3" size={16} aria-hidden="true" />
            <span>{project.active.name}</span>
          {:else}
            <span class="text-fg-2">{t('onboarding.done.noProject')}</span>
          {/if}
        </dd>
      </dl>
      <p class="inline-flex items-center gap-2 border-t border-line pt-3 text-sm text-fg-2"><Check class="text-success" size={16} aria-hidden="true" />{t('onboarding.done.private')}</p>
    </section>

    <section class="flex flex-col gap-3 rounded-lg border border-line bg-glass p-5" aria-labelledby="first-steps">
      <h3 id="first-steps" class="text-sm font-medium tracking-wider text-fg-3 uppercase">{t('onboarding.done.where')}</h3>
      <ul class="flex flex-col gap-3">
        {#each TIPS as tip (tip.keys)}
          <li class="grid grid-cols-[max-content_1fr] items-start gap-4">
            <span class="pt-0.5"><Kbd keys={tip.keys} uniform /></span>
            <div class="flex flex-col gap-0.5">
              <p class="text-sm font-medium text-fg">{tip.title}</p>
              <p class="text-sm text-fg-2">{tip.body}</p>
            </div>
          </li>
        {/each}
      </ul>
    </section>
  </div>
</div>
