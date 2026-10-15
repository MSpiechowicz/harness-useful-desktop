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
  import SetupCommand from './SetupCommand.svelte';
  import { isUnsupported, MCP_SNIPPET, mcpChip, refusalText, skillsChip, type SetupKey } from './setup-model';
  import { setup } from './setup-state.svelte';
  import { badgeOf, nameOf } from './setup-view';

  interface Props {
    entry: HarnessSetup;
  }

  let { entry }: Props = $props();

  const name = $derived(nameOf(entry.harness));
  const skillsKey = $derived(`skills:${entry.harness}` as SetupKey);
  const mcpKey = $derived(`mcp:${entry.harness}` as SetupKey);
  const skillsFailure = $derived(setup.failure(skillsKey));
  const mcpFailure = $derived(setup.failure(mcpKey));

  // What does not fit in a cell: only an installed harness has any of it.
  const manual = $derived(entry.installed && entry.skills.type === 'manual' ? entry.skills : null);
  const unavailable = $derived(entry.installed && entry.skills.type === 'unavailable' ? entry.skills : null);
  const refused = $derived(entry.installed && entry.mcp.type === 'refused' && !isUnsupported(entry.mcp) ? entry.mcp : null);
  const disabled = $derived(entry.installed && entry.mcp.type === 'disabled');
  const waitsForDashboard = $derived(entry.installed && entry.mcp.type === 'missing' && !setup.selectable(mcpKey));

  const hasSkillsNote = $derived(!!(manual || unavailable || skillsFailure));
  const hasMcpNote = $derived(!!(refused || disabled || waitsForDashboard || mcpFailure));
  const hasNotes = $derived(hasSkillsNote || hasMcpNote);

  /** Rows that need the user: something to run by hand, something refused, something that failed. */
  const needsAttention = $derived(!!(manual || refused || skillsFailure || mcpFailure));

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

  /** Where the harness keeps its MCP configuration: the hint of the MCP cell, not a line of the row. */
  const configFile = $derived(entry.installed && !isUnsupported(entry.mcp) ? entry.mcp.file : undefined);
</script>

<li class="border-t border-line">
  <div class={cx(SETUP_MATRIX, 'py-[9px]', !entry.installed && 'opacity-60')}>
    <div class="flex min-w-0 items-center gap-2.5">
      <HarnessBadge info={badgeOf(entry.harness)} size="tile" />

      <div class="min-w-0">
        <p class="truncate text-sm leading-5 font-medium text-fg" title={name}>{name}</p>
        <p class="truncate text-xs leading-4 text-fg-3" title={detail}>{detail || ' '}</p>
      </div>
    </div>

    <SetupCell
      label={t('onboarding.setup.skills.title')}
      chip={skillsChip(entry, setup.shown(skillsKey), setup.waiting(skillsKey))}
    />

    <SetupCell
      label={t('onboarding.setup.mcp.title')}
      hint={configFile}
      chip={mcpChip(entry, setup.shown(mcpKey), setup.waiting(mcpKey))}
    />

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
          {#if hasSkillsNote}
            <section class="flex flex-col gap-2" aria-label={t('onboarding.setup.skills.title')}>
              <h4 class="text-xs font-medium tracking-wider text-fg-3 uppercase">{t('onboarding.setup.skills.title')}</h4>

              {#if unavailable}
                <p class="text-sm text-fg-2">{unavailable.reason}</p>
              {:else if manual}
                <p class="text-sm text-fg-2">{manual.note}</p>
                <SetupCommand text={manual.command} copyLabel={t('onboarding.setup.copyCommand')} />
              {/if}

              {#if skillsFailure}
                <p class="text-sm text-danger" role="alert">{t('onboarding.setup.skills.failed', { reason: skillsFailure })}</p>
              {/if}
            </section>
          {/if}

          {#if hasMcpNote}
            <section class="flex flex-col gap-2" aria-label={t('onboarding.setup.mcp.title')}>
              <h4 class="text-xs font-medium tracking-wider text-fg-3 uppercase">{t('onboarding.setup.mcp.title')}</h4>

              {#if refused}
                <p class="text-sm text-fg-2">{t('onboarding.setup.mcp.refused', { reason: refusalText(refused.reason) })}</p>
                <SetupCommand text={MCP_SNIPPET} copyLabel={t('onboarding.setup.copySnippet')} />
              {:else if disabled}
                <p class="text-sm text-fg-2">{t('onboarding.setup.mcp.disabled')}</p>
              {:else if waitsForDashboard}
                <p class="text-sm text-fg-2">{t('onboarding.setup.mcp.needsDashboard')}</p>
              {/if}

              {#if mcpFailure}
                <p class="text-sm text-danger" role="alert">{t('onboarding.setup.mcp.failed', { reason: mcpFailure })}</p>
              {/if}
            </section>
          {/if}
        </div>
      </div>
    {/if}
  </div>
</li>
