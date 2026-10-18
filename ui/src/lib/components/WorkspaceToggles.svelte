<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  import PanelSideIcon from './PanelSideIcon.svelte';
  import { PILL_FRAME, PILL_RAISED } from './strip';
  import Tooltip from './Tooltip.svelte';
  import type { WorkspaceState } from './workspace.svelte';

  interface Props {
    workspace: WorkspaceState;
    /** What the list holds, e.g. "Sessions": the accessible name and tooltip of its toggle. */
    listLabel: string;
  }

  let { workspace, listLabel }: Props = $props();

  // The ring of a key press is drawn inside the segment, so it never reaches the one beside it. A click does not take focus at all: the segment is a switch, not a place to type.
  const SEGMENT = 'grid h-full w-8 place-items-center transition-colors duration-[var(--dur-fast)] focus-visible:outline-offset-[-2px]';
</script>

{#snippet segment(side: 'left' | 'right', label: string, shortcut: string, pressed: boolean, toggle: () => void, marker: Record<string, string>)}
  <Tooltip text={label} {shortcut} side="bottom" class="inline-flex h-full">
    {#snippet children({ describedby })}
      <button
        type="button"
        aria-label={label}
        aria-describedby={describedby}
        aria-pressed={pressed}
        data-workspace={workspace.id}
        {...marker}
        class={cx(SEGMENT, pressed ? cx(PILL_RAISED, 'text-fg') : 'rounded-sm border border-transparent text-fg-2 hover:text-fg')}
        onmousedown={(event) => event.preventDefault()}
        onclick={toggle}
      >
        <PanelSideIcon {side} filled={pressed} />
      </button>
    {/snippet}
  </Tooltip>
{/snippet}

<!-- The layout switch of the top bar: one segment group like the Structured / Source switch, but each segment toggles on its
     own, so both can be open. Place it after the view's primary action and before its overflow menu. Nothing shows until the
     view is laid out, so an empty state has no dead buttons. -->
{#if workspace.attached}
  <div role="group" aria-label={t('components.workspace.layout')} class={cx('inline-flex items-center', PILL_FRAME)}>
    {@render segment('left', listLabel, 'mod+[', workspace.listVisible, () => workspace.toggleList(), { 'data-drawer-toggle': '' })}

    {#if workspace.hasDetails}
      {@render segment('right', t('components.workspace.details'), 'mod+]', workspace.detailsVisible, () => workspace.toggleDetails(), { 'data-details-toggle': '' })}
    {/if}
  </div>
{/if}
