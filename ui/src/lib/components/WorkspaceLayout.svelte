<script lang="ts">
  import { type Snippet, untrack } from 'svelte';

  import { scrollMemory } from '$lib/a11y/scroll-memory';
  import { CENTRE_MIN_PX, LIST_MIN_PX } from '$lib/design/layout';
  import { DURATION, scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';
  import { escapeCloses } from '$lib/utils/escape-closes';

  import DetailEmpty from './DetailEmpty.svelte';
  import type { IconComponent } from './icon';
  import ListDrawer from './ListDrawer.svelte';
  import SidePanel from './SidePanel.svelte';
  import { SIDE_PANEL_WIDTH, sidePanelPx } from './side-panel';
  import SplitPane from './SplitPane.svelte';
  import type { WorkspaceState } from './workspace.svelte';

  interface Props {
    workspace: WorkspaceState;
    /** Accessible name of the list: of the drawer it becomes on a narrow window. */
    listLabel: string;
    /** Accessible name of the draggable divider after the list. */
    resizeLabel: string;
    /**
     * Who scrolls the centre, under its `header`. `page` makes it one page scroller with the page gutter and rhythm: callers
     * render their content, not a scroller. `own` leaves it a column that does not scroll, for a child that fills it and scrolls itself.
     */
    scroll: 'page' | 'own';
    /** Remembers the page scroll position under this key while `scroll` is `page`. Key it per item to remember each item's. */
    scrollKey?: string;
    /** Heading of the details panel; defaults to "Details". */
    detailsTitle?: string;
    /** One line under the heading: what is chosen. */
    detailsSubtitle?: string;
    /** Set while nothing is chosen: the panel shows this message instead of `details`. */
    detailsEmpty?: { icon: IconComponent; message: string };
    list: Snippet;
    /** The header band of the centre (a `PaneHeader`). It stays above the scroller, so its line runs edge to edge and the title stays while the page scrolls. */
    header?: Snippet;
    main: Snippet;
    /** The body of the details panel; the panel itself (header, close button, scrolling) is the layout's. */
    details?: Snippet;
  }

  let { workspace, listLabel, resizeLabel, scroll, scrollKey, detailsTitle, detailsSubtitle, detailsEmpty, list, header, main, details }: Props = $props();

  const page = $derived(scroll === 'page');
  const title = $derived(detailsTitle ?? t('components.workspace.details'));
  // The centre keeps its room even with the details column beside it.
  const minCentre = $derived(CENTRE_MIN_PX + (workspace.detailsInline && workspace.detailsVisible ? sidePanelPx() : 0));

  // The details column stays mounted while it animates away, so its content does not vanish before the column does.
  let detailsMounted = $state(untrack(() => workspace.detailsVisible));

  $effect(() => workspace.attach());

  $effect(() => {
    if (workspace.detailsVisible) {
      detailsMounted = true;

      return;
    }

    const timer = setTimeout(() => (detailsMounted = false), scaled(DURATION.base));

    return () => clearTimeout(timer);
  });
</script>

{#snippet listPane()}
  <div class="relative flex size-full flex-col overflow-hidden">{@render list()}</div>
{/snippet}

{#snippet detailsBody()}
  {#if detailsEmpty}
    <DetailEmpty icon={detailsEmpty.icon} message={detailsEmpty.message} />
  {:else}
    {@render details?.()}
  {/if}
{/snippet}

<!-- The centre is the header band over one scroller. The scroller is one element in both modes, only its classes change, so
     `main` keeps its place in the DOM and a switch between a page and a child that scrolls itself does not remount what the
     view put in it. -->
{#snippet centre()}
  <div class="@container relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
    {@render header?.()}

    <div
      use:scrollMemory={page ? scrollKey : undefined}
      class={cx('min-h-0 min-w-0 flex-1', page ? 'overflow-y-auto [scrollbar-gutter:stable_both-edges]' : 'flex flex-col overflow-hidden')}
    >
      <div class={page ? 'page-gutter py-6' : 'contents'}>{@render main()}</div>
    </div>
  </div>
{/snippet}

<!-- The details as a column of the view: it is always in place and only its width and opacity change, so the centre moves with it. -->
{#snippet detailsColumn()}
  <div
    data-workspace={workspace.id}
    data-workspace-panel="details"
    inert={!workspace.detailsVisible}
    aria-hidden={workspace.detailsVisible ? undefined : true}
    class="flex shrink-0 overflow-hidden transition-[width] duration-[var(--dur-base)] ease-[var(--ease-out-expo)]"
    style:width={workspace.detailsVisible ? SIDE_PANEL_WIDTH : '0'}
  >
    <div
      class="flex min-h-0 shrink-0 flex-col border-l border-panel-edge bg-surface-1 transition-opacity duration-[var(--dur-base)]"
      class:opacity-0={!workspace.detailsVisible}
      style:width={SIDE_PANEL_WIDTH}
    >
      <SidePanel inline open={detailsMounted} {title} subtitle={detailsSubtitle} onclose={() => workspace.closeDetails()}>
        {@render detailsBody()}
      </SidePanel>
    </div>
  </div>
{/snippet}

{#snippet beside()}
  <div class="flex size-full min-h-0 min-w-0">
    {@render centre()}

    {#if workspace.detailsInline}
      {@render detailsColumn()}
    {/if}
  </div>
{/snippet}

<!-- list | centre | details. Wide, the list and details are columns. Narrower, the list is a drawer and the details slide
     over the centre, both opened from `WorkspaceToggles`. Escape puts the details slide-over away from anywhere in the view. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  bind:clientWidth={workspace.width}
  class="relative size-full min-h-0"
  onkeydown={(event) => escapeCloses(event, workspace.overlayOpen, () => workspace.closeOverlay())}
>
  {#if workspace.listInline}
    <SplitPane label={resizeLabel} bind:size={workspace.listSize} resetTo={sidePanelPx()} collapsed={!workspace.listShown} min={LIST_MIN_PX} minSecond={minCentre}>
      {#snippet first(width)}
        <div data-workspace={workspace.id} data-workspace-panel="list" class="size-full bg-surface-1" style:width="{width}px">
          {@render listPane()}
        </div>
      {/snippet}

      {#snippet second()}
        {@render beside()}
      {/snippet}
    </SplitPane>
  {:else}
    {@render beside()}

    <ListDrawer drawer={workspace.drawer} label={listLabel}>
      <div data-workspace={workspace.id} data-workspace-panel="list" class="size-full">{@render listPane()}</div>
    </ListDrawer>
  {/if}

  {#if workspace.hasDetails && !workspace.detailsInline}
    <!-- `contents` takes no box of its own: the panel still positions itself against the view, and the wrapper only marks it as the details for focus hand-back. -->
    <div data-workspace={workspace.id} data-workspace-panel="details" class="contents">
      <SidePanel open={workspace.overlayOpen} {title} subtitle={detailsSubtitle} onclose={() => workspace.closeOverlay()}>
        {@render detailsBody()}
      </SidePanel>
    </div>
  {/if}
</div>
