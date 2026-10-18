<script lang="ts">
  import { HEADER_CONTROLS_SLOT, HEADER_FIXED_SLOT, HEADER_TITLE_SLOT } from '$lib/actions/portal';
</script>

<!-- The one bar above the content, filled by the current view's header (see ViewHeader). History is on Alt+Left and
     Alt+Right. Who and where you are (brand, harness, project) live on the rail. The three places keep this order,
     which is also the focus order. The first two are stacked cells, so the outgoing and incoming parts of a page switch
     cross in place. The last does not animate: a fixed end (the layout toggles and the "⋯" menu) that is leaving is
     inert and stays until the incoming one is ready (it has had a moment to fill in), then the incoming one takes its
     place in one step. The two look alike, so it seems not to have changed, and it does not blink off while the
     incoming view is still loading. Only one shows at a time: of several leaving ones the oldest (the one on screen),
     so a quick A to B to C never shows B's, which never was.
     53 px with the 1 px bottom line inside it leaves 52 px, so a 36 px control has 8 px above and below. -->
<header class="intro-enter intro-top flex h-[53px] shrink-0 items-center border-b border-line bg-surface-1/70 px-4" style:--intro-delay="60ms">
  <div id={HEADER_TITLE_SLOT} class="grid h-full min-w-min flex-1 grid-cols-[minmax(0,1fr)] items-center *:col-start-1 *:row-start-1"></div>
  <div id={HEADER_CONTROLS_SLOT} class="grid h-full min-w-0 shrink grid-cols-[minmax(0,1fr)] items-center justify-items-end pl-3 *:col-start-1 *:row-start-1"></div>
  <div id={HEADER_FIXED_SLOT} class="fixed-slot flex h-full shrink-0 items-center"></div>
</header>

<style>
  /*
   * Parts pile up in the slot in the order the views came and went. What is on screen is the oldest leaving one until a
   * newer part that is not leaving is ready. Meanwhile no other part shows: not another leaving one (a view that was
   * switched away from before it ever showed), not a leaving one that never got ready (it was never on screen, so it
   * must not appear when the older one is removed before the newest is ready), and not the incoming one.
   */
  .fixed-slot > :global([data-leaving]:not([data-ready])),
  .fixed-slot > :global([data-leaving] ~ [data-leaving]),
  .fixed-slot > :global([data-leaving]:has(~ :not([data-leaving])[data-ready])),
  .fixed-slot > :global([data-leaving] ~ :not([data-leaving]):not([data-ready])) {
    display: none;
  }
</style>
