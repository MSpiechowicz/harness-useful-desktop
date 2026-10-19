<script lang="ts">
  import Plus from '@lucide/svelte/icons/plus';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import { fade } from 'svelte/transition';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import { ICON } from '$lib/components/icon';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { scaled } from '$lib/design/motion';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { HookSeed } from '$lib/ipc/bindings/HookSeed';

  import SettingRow from '../settings/SettingRow.svelte';
  import DesktopMcpHint from './DesktopMcpHint.svelte';
  import FamilyInstall from './FamilyInstall.svelte';
  import { EVENT_ICONS, EVENT_LABELS, MEMBER_LABELS, memberOf } from './hook-format';
  import { hooks } from './hooks-data.svelte';

  interface Props {
    open: boolean;
  }

  let { open = $bindable(false) }: Props = $props();

  /** The seeds a change is being written for. */
  let working = $state<readonly HookSeed[]>([]);

  const rows = $derived(hooks.family);
  const missing = $derived((rows ?? []).filter((row) => row.status === 'missing').map((row) => row.seed));

  // Opening the dialog reads the catalog again: a hook may have been deleted or an agent may have changed one since.
  $effect(() => {
    if (open) {
      void hooks.readFamily();
    }
  });

  async function apply(seeds: HookSeed[]): Promise<void> {
    working = seeds;

    try {
      await hooks.applyFamily(seeds);
    } finally {
      working = [];
    }
  }

</script>

<Dialog bind:open size="lg" title={t('hooks.family.title')} description={t('hooks.family.description')}>
  {#if hooks.familyError && rows === null}
    <p class="text-sm text-danger" role="alert">{t('hooks.family.loadFailed', { reason: hooks.familyError })}</p>
  {:else if rows === null}
    <div class="flex flex-col gap-3 py-2" role="status" aria-label={t('hooks.family.loading')}>
      <Skeleton height="2.75rem" />
      <Skeleton height="2.75rem" />
      <Skeleton height="2.75rem" />
    </div>
  {:else}
    <div in:fade={{ duration: scaled(160) }} class="-my-4 divide-y divide-line">
      {#each rows as row (row.seed)}
        {@const Icon = EVENT_ICONS[row.event]}
        {@const member = memberOf(row.member)}

        <SettingRow title={row.name} description={t('hooks.family.row', { member: t(MEMBER_LABELS[member]), event: t(EVENT_LABELS[row.event]) })}>
          {#snippet leading()}
            <span class="grid size-control shrink-0 place-items-center rounded-md border border-line bg-surface-2 text-accent-text"><Icon size={ICON.sm} strokeWidth={1.8} aria-hidden="true" /></span>
          {/snippet}

          {#snippet control()}
            <div class="flex items-center justify-end gap-2.5">
              {#if row.status === 'installed'}
                <Badge tone="success" dot size="lg">{t('hooks.family.status.installed')}</Badge>
              {:else if row.status === 'needsMember'}
                <FamilyInstall {member} />
              {:else}
                {#if row.status === 'outdated'}<Badge tone="warning" size="lg">{t('hooks.family.status.outdated')}</Badge>{/if}
                <Button loading={working.includes(row.seed)} disabled={working.length > 0} onclick={() => void apply([row.seed])}>
                  {#snippet leading()}
                    {#if row.status === 'outdated'}<RefreshCw size={ICON.sm} aria-hidden="true" />{:else}<Plus size={ICON.sm} aria-hidden="true" />{/if}
                  {/snippet}
                  {t(row.status === 'outdated' ? 'hooks.family.update' : 'hooks.family.add')}
                </Button>
              {/if}
            </div>
          {/snippet}
        </SettingRow>
      {/each}
    </div>
  {/if}

  <div class="mt-4"><DesktopMcpHint onleave={() => (open = false)} /></div>

  {#snippet footer()}
    <Button variant="ghost" onclick={() => (open = false)}>{t('hooks.family.close')}</Button>
    <Button variant="primary" loading={working.length > 1} disabled={missing.length === 0 || working.length > 0} onclick={() => void apply(missing)}>
      {missing.length > 0 ? tn('hooks.family.addAll', missing.length) : t('hooks.family.addAll.none')}
    </Button>
  {/snippet}
</Dialog>
