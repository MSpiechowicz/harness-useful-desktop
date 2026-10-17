<script lang="ts">
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import { slide } from 'svelte/transition';

  import { scaled } from '$lib/design/motion';
  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HarnessSetup } from '$lib/ipc/bindings/HarnessSetup';
  import { harness } from '$lib/stores/harness.svelte';
  import { cx } from '$lib/utils/cx';
  import { uid } from '$lib/utils/ids';

  import { SETUP_MATRIX } from './layout';
  import SetupCell from './SetupCell.svelte';
  import SetupMcpNote from './SetupMcpNote.svelte';
  import { isUnsupported, mcpChip, mcpTitle, type McpKind, type SetupKey } from './setup-model';
  import type { MessageKey } from '$lib/i18n/locales/en';
  import { comingSoon, setup } from './setup-state.svelte';
  import { badgeOf, nameOf } from './setup-view';

  interface Props {
    entry: HarnessSetup;
  }

  let { entry }: Props = $props();

  const name = $derived(nameOf(entry.harness));
  /** Listed, greyed out and left alone until the app can chat with it. */
  const soon = $derived(comingSoon(entry.harness));

  /** What a missing entry says while the server it runs is not installed; the app's own entry runs the app and never waits. */
  const NEEDS: Record<McpKind, MessageKey | null> = { mcp: 'onboarding.setup.mcp.needsDashboard', memoryMcp: 'onboarding.setup.memoryMcp.needsMemory', desktopMcp: null };

  /** One MCP column of the row: its cell and what its note would say. The dashboard's, the memory server's and the app's own work alike. */
  function mcpColumn(kind: McpKind) {
    const mcp = entry[kind];
    const key: SetupKey = `${kind}:${entry.harness}`;
    const waits = entry.installed && mcp.type === 'missing' && !setup.selectable(key);

    const note = {
      refused: entry.installed && mcp.type === 'refused' && !isUnsupported(mcp) ? mcp.reason : null,
      manualEdit: entry.installed && mcp.type === 'manualEdit' ? { file: mcp.file, snippet: mcp.snippet } : null,
      disabled: entry.installed && mcp.type === 'disabled',
      waitsFor: waits && NEEDS[kind] ? t(NEEDS[kind]) : null,
      failure: setup.failure(key),
    };

    return {
      kind,
      key,
      title: mcpTitle(kind),
      // Where the harness keeps its MCP configuration: the hint of the cell, not a line of the row.
      file: entry.installed && !isUnsupported(mcp) ? mcp.file : undefined,
      note,
      hasNote: !soon && !!(note.refused || note.manualEdit || note.disabled || note.waitsFor || note.failure),
    };
  }

  const mcpColumns = $derived([mcpColumn('mcp'), mcpColumn('memoryMcp'), mcpColumn('desktopMcp')]);

  const hasNotes = $derived(mcpColumns.some((column) => column.hasNote));

  /** Rows that need the user: something refused or left to a manual edit, something that failed. */
  const needsAttention = $derived(mcpColumns.some((column) => column.hasNote && (column.note.refused || column.note.manualEdit || column.note.failure)));

  // A row starts open when it needs the user, and opens by itself when it comes to (a failure, a fresh reading of the machine);
  // closing it again sticks until the need changes.
  // svelte-ignore state_referenced_locally
  let open = $state(needsAttention);

  $effect(() => {
    if (needsAttention) {
      open = true;
    }
  });

  const panelId = uid('setup-row');

  /** The one line under the name: which version of the harness is installed, or what is missing. */
  const detail = $derived.by(() => {
    if (!entry.installed) {
      return t('onboarding.setup.skills.installFirst', { name });
    }

    const version = harness.get(entry.harness)?.detection.version;

    return version ? `v${version}` : '';
  });
</script>

<li class="border-t border-line">
  <div class={cx(SETUP_MATRIX, 'py-[9px]', (!entry.installed || soon) && 'opacity-60', soon && 'grayscale')}>
    <div class="flex min-w-0 items-center gap-2.5">
      <HarnessBadge info={badgeOf(entry.harness)} size="tile" />

      <div class="min-w-0">
        <p class="truncate text-sm leading-5 font-medium text-fg" title={name}>{name}</p>
        <p class="truncate text-xs leading-4 text-fg-3" title={detail}>{detail || ' '}</p>
      </div>
    </div>

    {#each mcpColumns as column (column.kind)}
      <SetupCell
        label={column.title}
        hint={soon ? undefined : column.file}
        chip={soon ? { tone: 'neutral', label: t('onboarding.card.comingSoon') } : mcpChip(entry, column.kind, setup.shown(column.key), setup.waiting(column.key))}
      />
    {/each}

    <span class="flex justify-end">
      {#if hasNotes}
        <button
          type="button"
          aria-expanded={open}
          aria-controls={panelId}
          aria-label={`${t('onboarding.setup.details')}: ${name}`}
          class="grid size-control place-items-center rounded-md text-fg-2 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg"
          onclick={() => (open = !open)}
        >
          <ChevronDown size={18} aria-hidden="true" class={cx('transition-transform duration-[var(--dur-base)] ease-[var(--ease-out-expo)]', open && 'rotate-180')} />
        </button>
      {/if}
    </span>
  </div>

  <div id={panelId}>
    {#if hasNotes && open}
      <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
        <div class="flex flex-col gap-4 pt-0.5 pb-3.5">
          {#each mcpColumns as column (column.kind)}
            {#if column.hasNote}
              <SetupMcpNote kind={column.kind} harness={entry.harness} title={column.title} {...column.note} />
            {/if}
          {/each}
        </div>
      </div>
    {/if}
  </div>
</li>
