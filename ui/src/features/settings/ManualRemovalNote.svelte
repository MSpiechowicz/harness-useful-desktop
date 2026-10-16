<script lang="ts">
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';

  import Button from '$lib/components/Button.svelte';
  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { ManualRemoval } from '$lib/ipc/bindings/ManualRemoval';

  import SetupCommand from '../onboarding/SetupCommand.svelte';
  import { badgeOf, nameOf } from '../onboarding/setup-view';

  interface Props {
    /** The entries project memory left behind: each sits in a file with comments, which the app does not rewrite. */
    removals: ManualRemoval[];
    ondismiss: () => void;
  }

  let { removals, ondismiss }: Props = $props();
</script>

<!-- A warning that stays until dismissed: unlike a toast it must still be there when the user opens the file. -->
<section class="flex flex-col gap-3 py-4" role="alert" aria-label={t('settings.memory.manual.title')}>
  <div class="flex items-center justify-between gap-6">
    <h3 class="flex items-center gap-2 text-base font-medium text-fg">
      <TriangleAlert class="size-4 shrink-0 text-warning" aria-hidden="true" />
      {t('settings.memory.manual.title')}
    </h3>
    <Button variant="ghost" onclick={ondismiss}>{t('settings.memory.manual.dismiss')}</Button>
  </div>

  {#each removals as removal (removal.harness + removal.file + removal.name)}
    <div class="flex flex-col gap-2">
      <div class="flex items-start gap-3">
        <HarnessBadge info={badgeOf(removal.harness)} size="xs" class="mt-0.5" />
        <p class="min-w-0 text-sm text-fg-2">{t('settings.memory.manual.body', { harness: nameOf(removal.harness), file: removal.file, name: removal.name })}</p>
      </div>
      <SetupCommand text={removal.name} copyLabel={t('settings.memory.manual.copy')} />
    </div>
  {/each}
</section>
