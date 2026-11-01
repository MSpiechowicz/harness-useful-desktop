<script lang="ts">
  import type { FamilyProduct } from '$lib/brand/family';
  import { t, type MessageKey } from '$lib/i18n/i18n.svelte';
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

  /** The MCP servers a harness can have, in the order of the columns. */
  const SERVERS: { product: FamilyProduct; title: MessageKey; body: MessageKey; note: MessageKey }[] = [
    { product: 'dashboard', title: 'onboarding.setup.mcp.title', body: 'onboarding.setup.mcp.body', note: 'onboarding.setup.mcp.note' },
    { product: 'memory', title: 'onboarding.setup.memoryMcp.title', body: 'onboarding.setup.memoryMcp.body', note: 'onboarding.setup.memoryMcp.note' },
    { product: 'knowledge', title: 'knowledge.onboarding.mcp.title', body: 'knowledge.onboarding.mcp.body', note: 'knowledge.onboarding.mcp.note' },
    { product: 'desktop', title: 'onboarding.setup.desktopMcp.title', body: 'onboarding.setup.desktopMcp.body', note: 'onboarding.setup.desktopMcp.note' },
  ];
</script>

<!-- Rows are harnesses, columns are the MCP servers that can be added to each. Whatever a cell cannot say goes in its row's disclosure, so every row is as tall as any other. -->
<section aria-labelledby={titleId} class="flex min-w-0 flex-col gap-1 rounded-lg border border-line bg-glass p-4 shadow-float [@media(max-height:720px)]:p-3.5">
  <div class={SETUP_MATRIX}>
    <h3 id={titleId} class="truncate text-xs font-medium tracking-wider text-fg-3 uppercase">{t('onboarding.setup.matrix.title')}</h3>

    {#each SERVERS as server (server.product)}
      <SetupColumnHead product={server.product} title={t(server.title)} />
    {/each}
    <span></span>
  </div>

  <ul class="mt-2 flex flex-col">
    {#each harnesses as entry (entry.harness)}
      <SetupMatrixRow {entry} />
    {/each}
  </ul>

  <SetupDetails>
    <div class="grid grid-cols-4 gap-6">
      {#each SERVERS as server (server.product)}
        <section class="flex flex-col gap-2" aria-label={t(server.title)}>
          <h4 class="flex items-center gap-2 text-sm font-medium text-fg"><Logo variant="mark" product={server.product} size={13} />{t(server.title)}</h4>
          <p>{t(server.body)}</p>
          <p class="text-fg-3">{t(server.note)}</p>
        </section>
      {/each}
    </div>
  </SetupDetails>
</section>
