<script lang="ts">
  import KeyRound from '@lucide/svelte/icons/key-round';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';

  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { SyncNote } from '$lib/ipc/bindings/SyncNote';

  import { keysLine, noteText } from './sync-model';

  interface Props {
    runsProgram: boolean;
    envKeys: string[];
    headerKeys: string[];
    notes: SyncNote[];
    /** The server is added by running the harness's CLI: the command that adds it and the harness's name. */
    viaCli?: { command: string; harness: string } | null;
    /** The server replaces one that is already there and keeps its env, headers and switch: its keys are not asked for again. */
    replacing?: boolean;
  }

  let { runsProgram, envKeys, headerKeys, notes, viaCli = null, replacing = false }: Props = $props();

  const keys = $derived(keysLine(envKeys, headerKeys, viaCli?.harness ?? null, replacing));
  // The version line says it already.
  const shown = $derived(notes.filter((note) => note !== 'versionInformational'));
</script>

{#if runsProgram}
  <!-- A stdio server is a program: it is the one thing in a setup that is highlighted, and it starts unticked. -->
  <div class="flex items-start gap-2.5 rounded-md border border-danger/40 bg-danger-wash px-3 py-2 text-sm" data-runs-program>
    <TriangleAlert size={ICON.sm} class="mt-0.5 shrink-0 text-danger" aria-hidden="true" />
    <div class="flex flex-col gap-0.5">
      <p class="font-medium text-danger">{t('sync.runs.title')}</p>
      <p class="text-fg-2">{t('sync.runs.body')}</p>
    </div>
  </div>
{/if}

{#if keys}
  <p class="flex items-start gap-2 text-sm text-fg-2">
    <KeyRound size={ICON.sm} class="mt-0.5 shrink-0 text-fg-3" aria-hidden="true" />
    {keys}
  </p>
{/if}

{#if shown.length > 0 || (viaCli && !replacing)}
  <ul class="flex flex-col gap-0.5 text-xs text-fg-3">
    {#if viaCli && !replacing}
      <li>{t('sync.note.addedByCli', { command: viaCli.command })}</li>
    {/if}
    {#each shown as note (note)}
      <li>{noteText(note)}</li>
    {/each}
  </ul>
{/if}
