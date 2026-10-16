<script lang="ts">
  import Package from '@lucide/svelte/icons/package';

  import Button from '$lib/components/Button.svelte';
  import Card from '$lib/components/Card.svelte';
  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { SkillInfo } from '$lib/ipc/bindings/SkillInfo';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { extensionsView } from '$lib/stores/views.svelte';

  import ActionBar from './actions/ActionBar.svelte';
  import { packageActions } from './actions/item-actions';
  import { packageOf } from './extension-items';

  interface Props {
    skill: SkillInfo;
  }

  let { skill }: Props = $props();

  // A skill that comes from a package is removed with the package, so the package's own actions are offered here too.
  const provider = $derived(extensionsView.data ? packageOf(extensionsView.data, skill) : undefined);
  const actions = $derived(
    provider?.tab === 'packages' && harness.active?.capabilities.cliActions ? packageActions(harness.active.info.id, provider.data, project.active?.path ?? null) : [],
  );

  function showPackage(): void {
    if (!provider) {
      return;
    }

    // Read before any write: the first write moves the detail off this skill, and this pane (with `provider`) goes away.
    const { tab, id } = provider;

    // A search that leaves the package out would send the detail to the first match instead.
    viewState.extensionsQuery = '';
    viewState.extensionsTab = tab;
    viewState.extensionsItem = id;
  }
</script>

{#if provider && skill.package}
  <Card as="section" padded={false} class="p-3.5">
    <h3 class="mb-2.5 text-xs font-medium tracking-wider text-fg-3 uppercase">{t('extensions.skill.package')}</h3>
    <div class="flex flex-col items-start gap-1.5">
      <Button variant="ghost" class="-ml-2.5" onclick={showPackage}>
        {#snippet leading()}<Package size={ICON.sm} aria-hidden="true" />{/snippet}
        {t('extensions.skill.fromPackage', { name: skill.package })}
      </Button>
      <p class="text-sm text-fg-2">{t('extensions.skill.packageNote')}</p>
    </div>
  </Card>

  {#if actions.length > 0}
    <ActionBar {actions} />
  {/if}
{/if}
