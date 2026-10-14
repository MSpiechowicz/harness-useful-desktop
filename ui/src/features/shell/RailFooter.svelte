<script lang="ts">
  import LangPicker from '$lib/components/LangPicker.svelte';
  import StatusDot from '$lib/components/StatusDot.svelte';
  import ThemeToggle from '$lib/components/ThemeToggle.svelte';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { app } from '$lib/stores/app.svelte';
  import { cx } from '$lib/utils/cx';

  import UpdateNotice from '../updates/UpdateNotice.svelte';
  import { railStatus } from './rail-status';

  interface Props {
    /** The rail shows labels. Collapsed, only the status dot remains. */
    expanded: boolean;
  }

  let { expanded }: Props = $props();

  const status = $derived(railStatus(app.status, agentSessions.active?.chat ?? null));
  const version = $derived(app.info?.version ?? '');
  const summary = $derived(version ? `${status.label} · v${version}` : status.label);
</script>

<!-- Language and theme, then the live status, lined up with the menu's highlight boxes. All sizes are pixels: this
     app's root font is 14px, the dashboard's 16px, and the footer matches the dashboard's. -->
<div class="mt-auto flex shrink-0 flex-col gap-[12px] border-t border-line px-[8px] py-[12px]">
  <UpdateNotice {expanded} />

  {#if expanded}
    <div class="flex items-center justify-between gap-[8px]">
      <LangPicker up />
      <ThemeToggle />
    </div>
  {/if}

  <!-- A fixed height: a longer status must not grow the footer and push the menu above it. -->
  <div class={cx('flex h-[16px] items-center gap-[8px] px-[4px] text-[11px] text-fg-3', !expanded && 'justify-center')} title={expanded ? undefined : summary}>
    <StatusDot tone={status.tone} live={status.live} />

    {#if expanded}
      <span class="min-w-0 flex-1 truncate" title={status.label}>{status.label}</span>
      {#if version}<span class="tabular shrink-0">v{version}</span>{/if}
    {/if}
  </div>
</div>
