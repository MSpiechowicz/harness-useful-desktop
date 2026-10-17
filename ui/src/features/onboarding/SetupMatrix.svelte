<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HarnessSetup } from '$lib/ipc/bindings/HarnessSetup';
  import Logo from '$lib/components/Logo.svelte';
  import { uid } from '$lib/utils/ids';

  import { SETUP_MATRIX } from './layout';
  import SetupColumnHead from './SetupColumnHead.svelte';
  import SetupDetails from './SetupDetails.svelte';
  import SetupMatrixRow from './SetupMatrixRow.svelte';

  interface Props {
    /** One row each, in the order the backend lists them: nothing here knows which harnesses exist. */
    harnesses: HarnessSetup[];
  }

  let { harnesses }: Props = $props();

  const titleId = uid('setup-matrix');
</script>

<!-- Rows are harnesses, columns are the MCP servers that can be added to each. Whatever a cell cannot say goes in its row's disclosure, so every row is as tall as any other. -->
<section aria-labelledby={titleId} class="flex min-w-0 flex-col gap-1 rounded-lg border border-line bg-glass p-4 shadow-float [@media(max-height:720px)]:p-3.5">
  <div class={SETUP_MATRIX}>
    <h3 id={titleId} class="truncate text-xs font-medium tracking-wider text-fg-3 uppercase">{t('onboarding.setup.matrix.title')}</h3>

    <SetupColumnHead product="dashboard" title={t('onboarding.setup.mcp.title')} />
    <SetupColumnHead product="memory" title={t('onboarding.setup.memoryMcp.title')} />
    <SetupColumnHead product="desktop" title={t('onboarding.setup.desktopMcp.title')} />
    <span></span>
  </div>

  <ul class="mt-2 flex flex-col">
    {#each harnesses as entry (entry.harness)}
      <SetupMatrixRow {entry} />
    {/each}
  </ul>

  <SetupDetails>
    <div class="grid grid-cols-3 gap-6">
      <section class="flex flex-col gap-2" aria-label={t('onboarding.setup.mcp.title')}>
        <h4 class="flex items-center gap-2 text-sm font-medium text-fg"><Logo variant="mark" product="dashboard" size={13} />{t('onboarding.setup.mcp.title')}</h4>
        <p>{t('onboarding.setup.mcp.body')}</p>
        <p class="text-fg-3">{t('onboarding.setup.mcp.note')}</p>
      </section>

      <section class="flex flex-col gap-2" aria-label={t('onboarding.setup.memoryMcp.title')}>
        <h4 class="flex items-center gap-2 text-sm font-medium text-fg"><Logo variant="mark" product="memory" size={13} />{t('onboarding.setup.memoryMcp.title')}</h4>
        <p>{t('onboarding.setup.memoryMcp.body')}</p>
        <p class="text-fg-3">{t('onboarding.setup.memoryMcp.note')}</p>
      </section>

      <section class="flex flex-col gap-2" aria-label={t('onboarding.setup.desktopMcp.title')}>
        <h4 class="flex items-center gap-2 text-sm font-medium text-fg"><Logo variant="mark" product="desktop" size={13} />{t('onboarding.setup.desktopMcp.title')}</h4>
        <p>{t('onboarding.setup.desktopMcp.body')}</p>
        <p class="text-fg-3">{t('onboarding.setup.desktopMcp.note')}</p>
      </section>
    </div>
  </SetupDetails>
</section>
