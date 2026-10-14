<script lang="ts">
  import Download from '@lucide/svelte/icons/download';

  import DropdownButton from '$lib/components/DropdownButton.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { InstallScope } from '$lib/ipc/bindings/InstallScope';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';

  import { actionRunner } from './action-runner.svelte';
  import { entryInstall } from './item-actions';

  interface Props {
    marketplace: string;
    entry: string;
  }

  let { marketplace, entry }: Props = $props();

  let open = $state(false);

  const projectPath = $derived(project.active?.path ?? null);

  function choose(scope: InstallScope, close: () => void): void {
    const active = harness.active;
    close();

    if (active) {
      void actionRunner.begin(entryInstall(active.info.id, marketplace, entry, scope, projectPath));
    }
  }
</script>

<Popover label={t('extensions.install.label', { entry })} kind="menu" align="end" bind:open class="mt-0! mb-2! bottom-full! top-auto! w-56 origin-bottom">
  {#snippet trigger({ props })}
    <DropdownButton {...props} icon={Download} disabled={actionRunner.phase !== 'idle'} aria-label={t('extensions.install.label', { entry })}>{t('extensions.install.button')}</DropdownButton>
  {/snippet}

  {#snippet children({ close })}
    <div role="none" class="flex flex-col">
      <button type="button" role="menuitem" data-menu-item class="rounded-sm px-2.5 py-2 text-left text-sm text-fg-2 transition-colors hover:bg-hover hover:text-fg focus-visible:bg-hover" onclick={() => choose('user', close)}>
        {t('extensions.install.forMe')}
        <span class="block text-xs text-fg-3">{t('extensions.install.userScope')}</span>
      </button>
      <button
        type="button"
        role="menuitem"
        data-menu-item
        disabled={projectPath === null}
        class="rounded-sm px-2.5 py-2 text-left text-sm text-fg-2 transition-colors hover:bg-hover hover:text-fg focus-visible:bg-hover disabled:cursor-not-allowed disabled:opacity-50"
        onclick={() => choose('project', close)}
      >
        {t('extensions.install.inProject')}
        <span class="block text-xs text-fg-3">{projectPath === null ? t('extensions.install.openFirst') : (project.active?.name ?? '')}</span>
      </button>
    </div>
  {/snippet}
</Popover>
