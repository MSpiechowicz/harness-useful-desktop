<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import ExternalLink from '@lucide/svelte/icons/external-link';
  import Info from '@lucide/svelte/icons/info';
  import { slide } from 'svelte/transition';

  import Button from '$lib/components/Button.svelte';
  import Disclosure from '$lib/components/Disclosure.svelte';
  import Logo from '$lib/components/Logo.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { ICON } from '$lib/components/icon';
  import { rise, scaled } from '$lib/design/motion';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { openExternal } from '$lib/ipc/actions';
  import type { EcosystemItem } from '$lib/ipc/bindings/EcosystemItem';
  import { toasts } from '$lib/stores/toasts.svelte';
  import { cx } from '$lib/utils/cx';
  import { uid } from '$lib/utils/ids';

  import SetupConfirm from '../onboarding/SetupConfirm.svelte';
  import { CompanionInstallFlow } from '../settings/family-install.svelte';
  import { cardTitleId, itemState, rowInstaller } from './ecosystem-items';
  import InlineCode from './InlineCode.svelte';
  import InstallSteps from './InstallSteps.svelte';
  import { productDescription, productNote, productTagline } from './product-copy';

  interface Props {
    item: EcosystemItem;
    /** Position in the list; rows enter one after another. */
    index?: number;
    open: boolean;
    ontoggle: () => void;
    /** Opens this row, whatever is open now. */
    onopen: () => void;
  }

  let { item, index = 0, open, ontoggle, onopen }: Props = $props();

  const status = $derived(itemState(item));
  const updating = $derived(status === 'installed' && item.updateAvailable);
  const titleId = $derived(cardTitleId(item.id));
  const panelId = uid('family-row');
  const actionLabel = $derived(t(updating ? 'ecosystem.row.update' : 'ecosystem.row.install'));
  const offersAction = $derived(status === 'missing' || updating);

  const stateLine = $derived.by(() => {
    if (updating && item.latestVersion) {
      return t('ecosystem.row.updateTo', { version: item.latestVersion });
    }

    if (status === 'installed') {
      return item.version ? `${t('ecosystem.card.installed')} · v${item.version}` : t('ecosystem.card.installed');
    }

    return t(status === 'missing' ? 'ecosystem.card.notInstalled' : 'ecosystem.card.unavailable');
  });

  // A lone step needs no count.
  const stepCount = $derived(item.installSteps.length > 1 ? tn('ecosystem.count.step', item.installSteps.length) : undefined);

  const installLabel = $derived.by(() => {
    if (updating) {
      return t('ecosystem.card.howToUpdate');
    }

    return status === 'installed' ? t('ecosystem.card.reinstall') : t('ecosystem.card.howToInstall');
  });

  // The secondary facts stay out of the way until asked for.
  let detailsOpen = $state(false);

  // A missing or outdated member shows its steps straight away; an up-to-date one tucks them away.
  let stepsOpen = $state(false);
  $effect.pre(() => {
    stepsOpen = status === 'missing' || updating;
  });

  // The list is keyed by id, so a row keeps one member for as long as it lives.
  // svelte-ignore state_referenced_locally
  const installer = rowInstaller(item.id);
  // The installer itself reloads the family, which is how this row learns the new state.
  const flow = installer ? new CompanionInstallFlow(installer, () => updating, () => Promise.resolve(), 'ecosystem.row.readFailed') : null;
  const busy = $derived(flow?.busy ?? false);

  /** A failure is told once, and the button is there to try again. */
  function tellFailure(wasUpdate: boolean): void {
    if (flow?.phase !== 'failed') {
      return;
    }

    toasts.push({ title: t(wasUpdate ? 'ecosystem.autoUpdate.failed' : 'ecosystem.autoInstall.failed', { name: item.name }), description: flow.failure || t('ecosystem.autoUpdate.noReason'), tone: 'danger' });
    flow.phase = 'idle';
  }

  /** The app runs the installer after the user confirmed its plan. A member without one opens on the steps to copy. */
  async function act(): Promise<void> {
    // Acting on a row chooses it, so it stays open once its state changes.
    onopen();

    if (!flow) {
      stepsOpen = true;

      return;
    }

    await flow.review();
    tellFailure(updating);
  }

  async function install(): Promise<void> {
    const wasUpdate = updating;

    await flow?.run();

    const outcome = flow?.outcome;

    // The automatic updater got there first: nothing failed, and the row follows when it is done.
    if (flow && outcome?.alreadyRunning) {
      toasts.push({ title: t('ecosystem.row.running.title', { name: item.name }), description: t('ecosystem.row.running.body'), tone: 'neutral' });
      flow.phase = 'idle';

      return;
    }

    if (flow?.phase === 'idle') {
      toasts.push({ title: t(wasUpdate ? 'ecosystem.autoUpdate.done' : 'ecosystem.autoInstall.done', { name: item.name }), description: doneBody(wasUpdate, outcome?.version ?? null), tone: 'success' });
    }

    tellFailure(wasUpdate);
  }

  function doneBody(wasUpdate: boolean, version: string | null): string {
    if (wasUpdate) {
      return version ? t('ecosystem.autoUpdate.doneBody', { version }) : t('ecosystem.autoUpdate.doneBodyLatest');
    }

    return version ? t('ecosystem.row.installedBody', { version }) : t('ecosystem.row.installedBodyPlain');
  }
</script>

<!-- Focusable by script only: choosing a module in the family diagram opens this row and brings it into view and focus. -->
<article
  aria-labelledby={titleId}
  tabindex="-1"
  in:rise={{ y: 14, delay: 120 + index * 80, duration: 420 }}
  class="col-span-full grid min-w-0 grid-cols-subgrid rounded-md outline-none focus-visible:outline-2 focus-visible:outline-offset-2"
>
  <div class="group relative col-span-full grid min-h-14 grid-cols-subgrid items-center gap-x-2.5 rounded-md px-2 py-1.5 transition-colors duration-[var(--dur-fast)] hover:bg-hover">
    <h3 class="min-w-0 pr-2.5 text-base font-normal">
      <!-- Its hit area covers the whole row; the install button sits above it. -->
      <button
        type="button"
        aria-expanded={open}
        aria-controls={panelId}
        onclick={ontoggle}
        class="flex w-full min-w-0 items-center gap-3 text-left outline-none after:absolute after:inset-0 after:rounded-md focus-visible:after:outline-2 focus-visible:after:-outline-offset-2 focus-visible:after:outline-[var(--focus)]"
      >
        <span class="grid h-control w-control shrink-0 place-items-center rounded-md border border-line bg-surface-2 shadow-raised" aria-hidden="true"><Logo variant="mark" product={item.id} size={16} /></span>
        <span class="flex min-w-0 flex-col">
          <!-- A longer button label in another language wraps the name instead of cutting it. -->
          <span id={titleId} class="font-medium text-fg">{item.name}</span>
          <span class="flex min-w-0 items-center gap-1.5 text-xs text-fg-3">
            <span class={cx('size-1.5 shrink-0 rounded-full', updating ? 'bg-accent-text' : status === 'installed' ? 'bg-success' : status === 'missing' ? 'bg-fg-3' : 'bg-warning')} aria-hidden="true"></span>
            <span class="tabular min-w-0">{stateLine}</span>
          </span>
        </span>
      </button>
    </h3>

    {#if offersAction}
      <Button
        bare
        variant="primary"
        class="z-10 col-start-2 w-full"
        disabled={busy}
        aria-busy={busy || undefined}
        aria-label={`${actionLabel} ${item.name}`}
        onclick={() => void act()}
      >
        <!-- The label keeps its room while the spinner shows, so no button changes width. -->
        {#if busy}<span class="absolute inset-0 grid place-items-center"><Spinner size={16} /></span>{/if}
        <span class={cx('label-trim', busy && 'invisible')}>{actionLabel}</span>
      </Button>
    {/if}

    <ChevronRight size={ICON.sm} aria-hidden="true" class={cx('pointer-events-none col-start-3 shrink-0 text-fg-3 transition-[transform,color] duration-[var(--dur-fast)] group-hover:text-fg', open && 'rotate-90')} />
  </div>

  <div id={panelId} class="col-span-full min-w-0">
    {#if open}
      <div transition:slide={{ duration: scaled(240) }} class="overflow-hidden">
        <div class="flex flex-col gap-3 px-2 pt-1 pb-2.5">
          <p class="text-sm text-fg-2">{productTagline(item)}</p>

          <div class="flex flex-wrap items-center gap-1.5">
            <Button onclick={() => void openExternal(item.repoUrl)}>
              {#snippet trailing()}<ExternalLink size={ICON.sm} aria-hidden="true" />{/snippet}
              GitHub
              <span class="sr-only">{t('ecosystem.card.repoSr', { name: item.name })}</span>
            </Button>

            {#if status === 'installed' && item.version}
              <span class="tabular ml-auto text-xs text-fg-3">
                v{item.version}{#if updating && item.latestVersion}<span class="mx-1" aria-hidden="true">→</span><span class="font-medium text-accent-text" aria-hidden="true">v{item.latestVersion}</span><span class="sr-only">{t('ecosystem.card.newer', { version: `v${item.latestVersion}` })}</span>{/if}
              </span>
            {/if}
          </div>

          {#if status === 'unsupported'}
            <p class="rounded-md border border-line bg-warning-wash px-3 py-2 text-sm text-fg" role="note">{t('ecosystem.card.unsupported', { name: item.name })}</p>
          {/if}

          <div class="flex flex-col gap-1 border-t border-line pt-2">
            <Disclosure label={t('ecosystem.card.details')} bind:open={detailsOpen}>
              <div class="flex flex-col gap-3 pb-2">
                {#if item.via.length > 0}
                  <dl class="flex items-baseline gap-3">
                    <dt class="w-16 shrink-0 text-xs text-fg-3">{t('ecosystem.card.foundAt')}</dt>
                    <dd class="flex min-w-0 flex-1 flex-col gap-1">
                      {#each item.via as found (found)}
                        <span class="truncate rounded-sm bg-surface-2 px-2 py-1 font-mono text-xs text-fg-2" title={found}>{found}</span>
                      {/each}
                    </dd>
                  </dl>
                {/if}

                <p class="text-sm text-fg-2"><InlineCode text={productDescription(item)} /></p>

                {#if item.notes.length > 0}
                  <ul class="flex flex-col gap-1.5">
                    {#each item.notes as _note, position (position)}
                      <li class="flex items-start gap-2 text-sm text-fg-3">
                        <Info size={16} class="mt-0.5 shrink-0" aria-hidden="true" />
                        <span class="min-w-0 [overflow-wrap:anywhere]"><InlineCode text={productNote(item, position)} /></span>
                      </li>
                    {/each}
                  </ul>
                {/if}
              </div>
            </Disclosure>

            {#if status !== 'unsupported' && item.installSteps.length > 0}
              <Disclosure label={installLabel} meta={stepCount} bind:open={stepsOpen}>
                <InstallSteps steps={item.installSteps} product={item.id} />
              </Disclosure>
            {/if}
          </div>
        </div>
      </div>
    {/if}
  </div>
</article>

{#if flow?.plan}
  <SetupConfirm
    bind:open={flow.confirming}
    plan={flow.plan}
    onconfirm={() => void install()}
    title={t(updating ? 'ecosystem.row.confirm.update' : 'ecosystem.row.confirm.install', { name: item.name })}
    body={t('ecosystem.row.confirm.body')}
    confirmLabel={actionLabel}
  />
{/if}
