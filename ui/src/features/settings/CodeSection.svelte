<script lang="ts">
  import { untrack } from 'svelte';
  import { slide } from 'svelte/transition';

  import Switch from '$lib/components/Switch.svelte';
  import { scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import { codeGraph } from '$lib/stores/codeGraph.svelte';
  import { CODE_STATUS_POLL_MS } from '$lib/stores/live';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { autoRefresh } from '$lib/utils/auto-refresh';

  import FamilyInstallRow from './FamilyInstallRow.svelte';
  import ManualRemovalNote from './ManualRemovalNote.svelte';
  import SettingRow from './SettingRow.svelte';
  import SettingsSection from './SettingsSection.svelte';

  interface Props {
    id?: string;
  }

  let { id }: Props = $props();

  const installable = $derived(codeGraph.installable);
  // The environment sets the switch for a server started with it: the row says so instead of letting a change through that would not stick.
  const description = $derived(t(codeGraph.locked ? 'settings.code.descriptionEnv' : 'settings.code.description'));
  /** Without a readable index (Harness Code missing, too old or not read yet) nothing is claimed: the switch shows off and waits for the install row. */
  const known = $derived(codeGraph.index !== null);
  /** On when the server is registered with the harnesses and its index is on. */
  const on = $derived(known && (prefs.value?.codeEnabled ?? true) && codeGraph.index?.enabled === true);

  // The server's state is read when the section opens and, while it is missing or old, again every few seconds: it may be getting installed elsewhere.
  $effect(() => {
    untrack(() => void codeGraph.load());

    if (installable === null) {
      return undefined;
    }

    return autoRefresh(() => codeGraph.load(), CODE_STATUS_POLL_MS);
  });
</script>

<SettingsSection {id} title={t('settings.code.section')}>
  <SettingRow title={t('settings.code.title')} {description}>
    {#snippet control()}
      <Switch label={t('settings.code.title')} disabled={!known || codeGraph.pending || prefs.codePending || codeGraph.locked} bind:checked={() => on, (enabled) => void codeGraph.setEnabled(enabled)} />
    {/snippet}
    {#snippet below()}
      {#if codeGraph.failure}
        <p transition:slide={{ duration: scaled(200) }} class="pt-2 text-sm text-danger" role="alert">{codeGraph.failure}</p>
      {/if}
    {/snippet}
  </SettingRow>

  {#if prefs.codeManualRemovals.length > 0}
    <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
      <ManualRemovalNote removals={prefs.codeManualRemovals} ondismiss={() => prefs.dismissCodeManualRemovals()} />
    </div>
  {/if}

  {#if installable !== null}
    <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
      <FamilyInstallRow member="code" update={installable === 'update'} onfinished={() => codeGraph.load()} />
    </div>
  {/if}
</SettingsSection>
