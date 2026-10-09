<script lang="ts">
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { app } from '$lib/stores/app.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { cx } from '$lib/utils/cx';

  import { chatStatus } from '../chat/chat-status';

  const connected = $derived(app.status === 'ready');
  const chat = $derived(agentSessions.active?.chat ?? null);
  const agent = $derived(chat ? chatStatus(chat) : null);
  const label = $derived(
    app.status === 'ready'
      ? (agent?.label ?? 'Ready · no agent running')
      : app.status === 'loading'
        ? 'Connecting…'
        : 'Desktop runtime unavailable',
  );
  const dot = $derived(!connected || agent?.tone === 'warning' ? 'bg-warning' : agent?.tone === 'danger' ? 'bg-danger' : 'bg-success');
</script>

<footer class="intro-enter intro-top flex h-7 shrink-0 items-center justify-between border-t border-line bg-surface-1/70 px-4 text-xs text-fg-3" style:--intro-delay="140ms">
  <span class="inline-flex items-center gap-2" role="status">
    <span class={cx('size-1.5 rounded-full', dot)} aria-hidden="true"></span>
    {label}
  </span>

  <span class="tabular flex items-center gap-3">
    {#if harness.active}
      <span>{harness.active.info.shortName}{harness.active.detection.version ? ` v${harness.active.detection.version}` : ''}</span>
    {/if}
    <span>{project.active ? project.active.name : 'No project'}</span>
    {#if app.info}
      <span>{app.info.name} v{app.info.version}</span>
    {/if}
  </span>
</footer>
