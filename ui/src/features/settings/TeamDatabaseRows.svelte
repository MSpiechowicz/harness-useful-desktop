<script lang="ts">
  import type { Snippet } from 'svelte';
  import { slide } from 'svelte/transition';

  import Button from '$lib/components/Button.svelte';
  import Switch from '$lib/components/Switch.svelte';
  import { scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import { IpcFailure } from '$lib/ipc/transport';
  import { cx } from '$lib/utils/cx';

  import SettingRow from './SettingRow.svelte';
  import type { TeamDatabaseCopy } from './remote-database';

  interface Props {
    /** What the rows say, in the words of the Harness family member whose team database it is. */
    copy: TeamDatabaseCopy;
    /** Whether a team database is saved. The switch follows it whenever it changes. */
    saved: boolean;
    /** The saved settings were read: the switch waits for them. */
    loaded: boolean;
    /** The saved settings could not be read, and there is nothing to show instead. */
    loadFailed: boolean;
    /** The member is switched off: the saved connection stays as it is, greyed out. */
    off: boolean;
    /** Removes the saved team database. A rejection is shown beside its confirmation. */
    turnOff: () => Promise<void>;
    /** The connection form, shown while the switch is on and the saved settings are read. */
    panel: Snippet;
  }

  let { copy, saved, loaded, loadFailed, off, turnOff, panel }: Props = $props();

  /** The switch position: it can be on before anything is saved, while the connection is still being entered. */
  let wanted = $state(false);
  let confirmingOff = $state(false);
  let turningOff = $state(false);
  let offFailure = $state<string | null>(null);

  // Follow the backend: what is saved decides the switch whenever it changes.
  $effect(() => {
    wanted = saved;
    confirmingOff = false;
  });

  function toggle(next: boolean): void {
    offFailure = null;

    if (next) {
      confirmingOff = false;
      wanted = true;

      return;
    }

    // Switching off something that is saved changes every teammate's data, so it asks first.
    if (saved) {
      confirmingOff = !confirmingOff;

      return;
    }

    wanted = false;
  }

  async function confirmOff(): Promise<void> {
    turningOff = true;
    offFailure = null;

    try {
      await turnOff();
    } catch (error) {
      offFailure = IpcFailure.from(error).message;
    } finally {
      turningOff = false;
    }
  }
</script>

<SettingRow title={t(copy.title)} description={t(copy.description)}>
  {#snippet control()}
    <Switch label={t(copy.title)} disabled={!loaded || turningOff || off} bind:checked={() => wanted, toggle} />
  {/snippet}
</SettingRow>

{#if loadFailed}
  <p class="py-4 text-sm text-danger" role="alert">{t(copy.loadFailed)}</p>
{/if}

{#if confirmingOff}
  <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
    <div class="flex items-center justify-between gap-6 py-4" role="alertdialog" aria-label={t(copy.offTitle)}>
      <div class="flex min-w-0 flex-col gap-0.5">
        <p class="text-base font-medium text-fg">{t(copy.offTitle)}</p>
        <p class="text-sm text-fg-2">{t(copy.offBody)}</p>
        {#if offFailure}<p class="text-sm text-danger" role="alert">{offFailure}</p>{/if}
      </div>
      <div class="flex shrink-0 items-center gap-2">
        <Button variant="ghost" disabled={turningOff} onclick={() => (confirmingOff = false)}>{t(copy.offCancel)}</Button>
        <Button variant="dangerQuiet" loading={turningOff} onclick={() => void confirmOff()}>{t(copy.offConfirm)}</Button>
      </div>
    </div>
  </div>
{/if}

{#if wanted && loaded}
  <!-- With the member off the saved connection stays as it is, only greyed out, so turning it back on just works. -->
  <div transition:slide={{ duration: scaled(200) }} class={cx('overflow-hidden transition-opacity duration-[var(--dur-fast)]', off && 'opacity-50')} inert={off}>
    {@render panel()}
  </div>
{/if}
