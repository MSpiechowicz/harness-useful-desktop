<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import Plus from '@lucide/svelte/icons/plus';

  import { ICON } from '$lib/components/icon';
  import IconButton from '$lib/components/IconButton.svelte';
  import Switch from '$lib/components/Switch.svelte';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HookScope } from '$lib/ipc/bindings/HookScope';
  import type { HookView } from '$lib/ipc/bindings/HookView';
  import { cx } from '$lib/utils/cx';

  import { ACTION_LABELS, EVENT_ICONS, EVENT_LABELS, hookTitle, type HookGroup, MAX_PER_SCOPE, SCOPE_ICONS, SCOPE_LABELS } from './hook-format';
  import HookStatusBadge from './HookStatusBadge.svelte';

  interface Props {
    groups: HookGroup[];
    /** The hook whose panel is open. */
    openId: string | null;
    /** The name of the open project, which the project group is titled with. */
    projectName: string | null;
    /** Hooks a change is being written for: their switch waits. */
    busy: ReadonlySet<string>;
    onopen: (id: string) => void;
    /** Moves the open panel along when the arrow keys move the focus. */
    onmove: (id: string) => void;
    ontoggle: (view: HookView, enabled: boolean) => void;
    onadd: (scope: HookScope) => void;
  }

  let { groups, openId, projectName, busy, onopen, onmove, ontoggle, onadd }: Props = $props();

  let body: HTMLElement | undefined = $state();

  const first = $derived(groups.flatMap((group) => group.views)[0]?.id ?? null);
  const stop = $derived(groups.some((group) => group.views.some((view) => view.id === openId)) ? openId : first);

  /** Why a row's switch is off, or null when it works: a hook that waits for approval or cannot run is never turned on by a click, as that would approve what the user has not seen. */
  function lockReason(view: HookView): string | null {
    if (view.status === 'pending') {
      return t('hooks.locked.pending');
    }

    return view.hook === null ? t('hooks.locked.invalid') : null;
  }

  function rows(): HTMLElement[] {
    return [...(body?.querySelectorAll<HTMLElement>('[data-row]') ?? [])];
  }

  /** Puts the focus on a row, e.g. back on the one whose panel was just closed. */
  export function focusRow(id: string): void {
    body?.querySelector<HTMLElement>(`[data-row="${CSS.escape(id)}"]`)?.focus({ preventScroll: false });
  }

  function onKeydown(event: KeyboardEvent, id: string): void {
    if (event.target !== event.currentTarget) {
      return;
    }

    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      onopen(id);

      return;
    }

    const all = rows();
    const at = all.findIndex((row) => row.dataset.row === id);
    const targets: Record<string, number> = { ArrowDown: at + 1, ArrowUp: at - 1, Home: 0, End: all.length - 1 };
    const target = all[targets[event.key]];

    if (!target) {
      return;
    }

    event.preventDefault();
    target.focus();
    onmove(target.dataset.row ?? id);
  }
</script>

{#snippet heading(group: HookGroup)}
  {@const scope = group.id === 'invalid' ? null : group.id}

  <tr>
    <th colspan="6" scope="colgroup" class="h-11 border-b border-line/60 bg-surface-2/55 py-0 pr-2 pl-3 text-left text-[11px] leading-[16px] font-medium tracking-wider text-fg-3 uppercase">
      <div class="flex items-center gap-2">
        <span class="flex min-w-0 flex-1 items-center gap-1.5">
          {#if scope}
            {@const Icon = SCOPE_ICONS[scope]}
            <Icon size={14} aria-hidden="true" class="shrink-0" />
            <!-- Label, project name and count share one baseline, whatever their font. -->
            <span class="flex min-w-0 items-baseline gap-1.5">
              <span class="shrink-0">{t(SCOPE_LABELS[scope])}</span>
              {#if scope === 'project' && projectName}<span class="min-w-0 truncate font-mono text-xs tracking-normal normal-case">{projectName}</span>{/if}
              <span class="tabular ml-1.5 shrink-0 text-xs tracking-wide normal-case">{t('hooks.list.groupCount', { count: group.total, max: MAX_PER_SCOPE })}</span>
            </span>
          {:else}
            <span class="flex min-w-0 items-baseline gap-1.5">
              <span class="truncate">{t('hooks.group.invalid')}</span>
              <span class="tabular ml-1.5 shrink-0 text-xs tracking-wide normal-case">{group.total}</span>
            </span>
          {/if}
        </span>

        {#if scope}
          <IconButton
            icon={Plus}
            label={t(scope === 'global' ? 'hooks.list.addGlobal' : 'hooks.list.addProject')}
            disabled={group.total >= MAX_PER_SCOPE || (scope === 'project' && !projectName)}
            onclick={() => onadd(scope)}
          />
        {/if}
      </div>
    </th>
  </tr>
{/snippet}

<!-- A table of the hooks in their scopes: every row is one stop of the Tab key, the arrow keys move between rows, and a row opens its panel.
     The switch of a row stays out of the way of that: it works the hook without opening it. A chevron ends every row, as it does in the Memory tables. -->
<div class="@container min-h-0 min-w-0 flex-1 overflow-y-auto overscroll-contain [scrollbar-gutter:stable]">
  <table role="grid" aria-label={t('hooks.list.label')} class="w-full table-fixed border-separate border-spacing-0 text-sm">
    <colgroup>
      <col />
      <col class="w-[17rem]" />
      <col class="hidden w-[8.5rem] @min-[780px]:table-column" />
      <col class="w-[10.5rem]" />
      <col class="w-[5.5rem]" />
      <col class="w-10" />
    </colgroup>

    <thead class="sticky top-0 z-10 bg-surface-1">
      <tr class="h-9 text-left text-[11px] leading-[16px] font-medium tracking-wide text-fg-3 uppercase">
        <th scope="col" class="border-b border-line px-3 text-left font-medium">{t('hooks.list.column.hook')}</th>
        <th scope="col" class="border-b border-line px-3 text-left font-medium">{t('hooks.list.column.when')}</th>
        <th scope="col" class="hidden border-b border-line px-3 text-left font-medium @min-[780px]:table-cell">{t('hooks.list.column.does')}</th>
        <th scope="col" class="border-b border-line px-3 text-left font-medium">{t('hooks.list.column.status')}</th>
        <th scope="col" class="border-b border-line px-3 text-right font-medium">{t('hooks.list.column.on')}</th>
        <th scope="col" aria-hidden="true" class="border-b border-line"></th>
      </tr>
    </thead>

    <tbody bind:this={body}>
      {#each groups as group (group.id)}
        {@render heading(group)}

        {#each group.views as view, index (view.id)}
          {@const hook = view.hook}
          {@const current = view.id === openId}
          {@const WhenIcon = hook ? EVENT_ICONS[hook.event] : null}

          <tr
            data-row={view.id}
            tabindex={view.id === stop ? 0 : -1}
            aria-selected={current}
            style:height="44px"
            style:--i={index}
            class={cx(
              'group/row cursor-pointer outline-offset-[-2px] transition-colors duration-[var(--dur-fast)] focus-visible:outline-2',
              current ? 'bg-accent-wash' : 'hover:bg-hover active:bg-press',
            )}
            onclick={() => onopen(view.id)}
            onkeydown={(event) => onKeydown(event, view.id)}
          >
            <td role="gridcell" class="border-b border-line/60 px-3">
              <div class={cx('truncate font-medium', view.status === 'off' ? 'text-fg-2' : 'text-fg')} title={hookTitle(view)}>{hookTitle(view)}</div>
            </td>

            <td role="gridcell" class="border-b border-line/60 px-3 text-fg-2">
              {#if hook && WhenIcon}
                <div class="flex min-w-0 items-center gap-1.5">
                  <WhenIcon size={ICON.sm} aria-hidden="true" class="shrink-0 text-fg-3" />
                  <span class="truncate">{t(EVENT_LABELS[hook.event])}</span>
                  {#if hook.matcher}<span class="h-5 max-w-[7rem] truncate rounded-sm bg-surface-3 px-1.5 font-mono text-xs leading-5 text-fg-2" title={t('hooks.field.toolFilter')}>{hook.matcher}</span>{/if}
                </div>
              {:else}
                <span class="text-fg-3">–</span>
              {/if}
            </td>

            <td role="gridcell" class="hidden border-b border-line/60 px-3 text-fg-2 @min-[780px]:table-cell">
              <div class="truncate">{#if hook}{t(ACTION_LABELS[hook.action.type])}{:else}–{/if}</div>
            </td>

            <td role="gridcell" class="border-b border-line/60 px-3"><HookStatusBadge status={view.status} /></td>

            <td role="gridcell" class="border-b border-line/60 px-3 text-right">
              <!-- The switch is its own control: a click or key on it must not open the row. A hook that waits for approval has it off, since a write approves what it writes. -->
              <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
              <span class="inline-flex" onclick={(event) => event.stopPropagation()} onkeydown={(event) => event.stopPropagation()}>
                <Tooltip text={lockReason(view) ?? ''} side="top" disabled={lockReason(view) === null}>
                  {#snippet children({ describedby })}
                    <span aria-describedby={describedby} class="inline-flex">
                      <Switch
                        label={t(hook?.enabled ? 'hooks.switch.off' : 'hooks.switch.on', { name: hookTitle(view) })}
                        disabled={lockReason(view) !== null || busy.has(view.id)}
                        bind:checked={() => view.status !== 'pending' && view.status !== 'invalid' && (hook?.enabled ?? false), (next) => ontoggle(view, next)}
                      />
                    </span>
                  {/snippet}
                </Tooltip>
              </span>
            </td>

            <td aria-hidden="true" class="border-b border-line/60 pr-3 text-right">
              <ChevronRight size={ICON.sm} class={cx('inline-block transition-colors duration-[var(--dur-fast)]', current ? 'text-accent-text' : 'text-fg-3 group-hover/row:text-fg-2')} />
            </td>
          </tr>
        {/each}

        {#if group.views.length === 0}
          <tr>
            <td colspan="6" class="h-11 border-b border-line/60 px-3 text-sm text-fg-3">
              {group.id === 'project' && !projectName ? t('hooks.list.noProject') : t('hooks.list.groupEmpty')}
            </td>
          </tr>
        {/if}
      {/each}
    </tbody>
  </table>
</div>

<style>
  /* Rows come in one after the other, a short rise each. Only the contents of the cells move, as a row itself cannot be transformed everywhere. */
  @keyframes row-in {
    from {
      opacity: 0;
      transform: translateY(var(--shift));
    }
  }

  tr[data-row] > td > * {
    animation: row-in var(--dur-enter) var(--ease-out-expo) both;
    animation-delay: calc(min(var(--i, 0), 10) * 28ms * var(--stagger));
  }
</style>
