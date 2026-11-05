<script lang="ts">
  import Download from '@lucide/svelte/icons/download';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import Webhook from '@lucide/svelte/icons/webhook';
  import { fade } from 'svelte/transition';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import { ICON } from '$lib/components/icon';
  import LoadingSpinner from '$lib/components/LoadingSpinner.svelte';
  import { scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HookFamilyEntry } from '$lib/ipc/bindings/HookFamilyEntry';

  import SessionConfirm from '../sessions/SessionConfirm.svelte';
  import SettingRow from '../settings/SettingRow.svelte';
  import DesktopMcpHint from './DesktopMcpHint.svelte';
  import { cadenceLabel, EVENT_ICONS, MEMBER_LABELS, memberOf } from './hook-format';
  import { hooks } from './hooks-data.svelte';
  import MemberInstall from './MemberInstall.svelte';

  interface Props {
    open: boolean;
  }

  let { open = $bindable(false) }: Props = $props();

  /** The hook (by seed) a change is being written for: its button works, the others wait. */
  let working = $state<HookFamilyEntry['seed'] | null>(null);
  let confirmer: SessionConfirm | undefined = $state();

  // A hook the catalog merged into another is listed only while its untouched file waits to be removed.
  const rows = $derived(hooks.family?.filter((row) => !row.retired || row.stale) ?? null);

  // The catalog numbers its steps 10, 20, ...: the badge counts them from 1 in that order, as the agent reads them. The message leaves out a step with nothing to say (no limit close, a branch up to date), so the numbers there can shift.
  const positions = $derived([...new Set(rows?.flatMap((row) => (row.step === null || row.retired ? [] : [row.step])))].sort((a, b) => a - b));
  const stepNumber = (position: number): number => positions.indexOf(position) + 1;

  /** What a row says about when it runs: a merged hook says it is going away. */
  const whenOf = (row: HookFamilyEntry): string => t(row.retired ? 'hooks.family.retired' : cadenceLabel(row.event));
  const describe = (row: HookFamilyEntry): string => t('hooks.family.row', { member: t(MEMBER_LABELS[memberOf(row.member)]), event: whenOf(row) });

  // Opening the dialog reads the catalog again: a hook may have been deleted or an agent may have changed one since.
  $effect(() => {
    if (open) {
      void hooks.readFamily();
    }
  });

  /** Installs a missing hook or updates an outdated one. A hook the user edited is only replaced after they confirm, as the update drops their changes. */
  async function apply(row: HookFamilyEntry): Promise<void> {
    if (row.status === 'outdated' && !row.stale) {
      const confirmed = await confirmer?.ask({
        title: t('hooks.family.overwrite.title'),
        description: t('hooks.family.overwrite.body'),
        subject: { icon: Webhook, name: row.name, details: describe(row) },
        confirmLabel: t('hooks.family.overwrite.confirm'),
        cancelLabel: t('hooks.family.overwrite.keep'),
      });

      if (!confirmed) {
        return;
      }
    }

    working = row.seed;

    try {
      await hooks.applyFamily([row.seed]);
    } finally {
      working = null;
    }
  }
</script>

<Dialog bind:open size="lg" title={t('hooks.family.title')} description={t('hooks.family.description')}>
  {#if hooks.familyError && rows === null}
    <p class="text-sm text-danger" role="alert">{t('hooks.family.loadFailed', { reason: hooks.familyError })}</p>
  {:else if rows === null}
    <LoadingSpinner label={t('hooks.family.loading')} class="h-40" />
  {:else}
    <div in:fade={{ duration: scaled(160) }} class="-my-4 divide-y divide-line">
      {#each rows as row (row.seed)}
        {@const Icon = EVENT_ICONS[row.event]}
        {@const member = memberOf(row.member)}

        <SettingRow title={row.name} description={describe(row)}>
          {#snippet leading()}
            <!-- The icon box is the same on every row. A step also carries its number, in the order the agent works through them. -->
            <span class="relative shrink-0">
              <span class="grid size-control place-items-center rounded-md border border-line bg-surface-2 text-accent-text"><Icon size={ICON.sm} strokeWidth={1.8} aria-hidden="true" /></span>
              {#if row.step !== null}
                <Badge tone="accent" class="absolute -top-2 -left-2"><span aria-hidden="true">{stepNumber(row.step)}</span><span class="sr-only">{t('hooks.family.step', { n: stepNumber(row.step) })}</span></Badge>
              {/if}
            </span>
          {/snippet}

          {#snippet control()}
            <!-- One thing on the right in every state, in a column as wide for the button as for the chip. -->
            <div class="flex min-w-32 items-center justify-end">
              {#if row.status === 'installed'}
                <Badge tone="success" dot size="lg">{t('hooks.family.status.installed')}</Badge>
              {:else if row.status === 'needsMember'}
                <MemberInstall {member} />
              {:else}
                <Button class="w-32" loading={working === row.seed} disabled={working !== null} aria-label={`${t(row.status === 'outdated' ? 'hooks.family.update' : 'hooks.family.installHook')} ${row.name}`} onclick={() => void apply(row)}>
                  {#snippet leading()}
                    {#if row.status === 'outdated'}<RefreshCw size={ICON.sm} aria-hidden="true" />{:else}<Download size={ICON.sm} aria-hidden="true" />{/if}
                  {/snippet}
                  {t(row.status === 'outdated' ? 'hooks.family.update' : 'hooks.family.installHook')}
                </Button>
              {/if}
            </div>
          {/snippet}
        </SettingRow>
      {/each}
    </div>
  {/if}

  <!-- Recall is the one hook that sends the user's message to a tool, so it says so where it is installed. -->
  {#if rows?.some((row) => row.seed === 'knowledge-first' && row.status !== 'needsMember')}
    <p class="mt-4 text-sm text-fg-2">{t('hooks.family.recallNote')}</p>
  {/if}

  <div class="mt-4"><DesktopMcpHint centered={false} onleave={() => (open = false)} /></div>

  {#snippet footer()}
    <Button variant="ghost" onclick={() => (open = false)}>{t('hooks.family.close')}</Button>
  {/snippet}
</Dialog>

<SessionConfirm bind:this={confirmer} />
