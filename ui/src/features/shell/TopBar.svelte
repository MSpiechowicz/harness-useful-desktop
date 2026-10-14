<script lang="ts">
  import { SHELL_HEADER_SLOT } from '$lib/actions/portal';
</script>

<!-- The one bar above the content: the current view's own header (title, state, search, actions). History is on
     Alt+Left and Alt+Right. Who and where you are (brand, harness, project) live on the rail. -->
<div class="intro-enter intro-top flex h-[52px] shrink-0 items-center gap-2 border-b border-line bg-surface-1/70 px-2" style:--intro-delay="60ms">
  <!-- Stacked cell: during a page switch the outgoing and incoming headers crossfade in place. -->
  <div id={SHELL_HEADER_SLOT} class="grid h-full min-w-0 flex-1 grid-cols-[minmax(0,1fr)] *:col-start-1 *:row-start-1"></div>
</div>
