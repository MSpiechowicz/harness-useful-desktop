<script lang="ts">
  import ArrowDown from '@lucide/svelte/icons/arrow-down';
  import ArrowUp from '@lucide/svelte/icons/arrow-up';
  import Download from '@lucide/svelte/icons/download';
  import GitBranch from '@lucide/svelte/icons/git-branch';
  import Keyboard from '@lucide/svelte/icons/keyboard';
  import Monitor from '@lucide/svelte/icons/monitor';
  import Moon from '@lucide/svelte/icons/moon';
  import Sun from '@lucide/svelte/icons/sun';
  import Zap from '@lucide/svelte/icons/zap';

  import { ICON } from '$lib/components/icon';
  import StatusDot from '$lib/components/StatusDot.svelte';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { app } from '$lib/stores/app.svelte';
  import { git } from '$lib/stores/git.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { updates } from '$lib/stores/updates.svelte';
  import { cx } from '$lib/utils/cx';

  import { cycleMotion, cycleTheme } from './actions';
  import { chatStatus } from '../chat/chat-status';
  import { viewUpdates } from '../updates/actions';
  import { indicatorLabel } from '../updates/copy';
  import StatusItem from './StatusItem.svelte';

  interface Props {
    onshortcuts: () => void;
  }

  let { onshortcuts }: Props = $props();

  const connected = $derived(app.status === 'ready');
  const chat = $derived(agentSessions.active?.chat ?? null);
  const agent = $derived(chat ? chatStatus(chat) : null);
  const label = $derived(
    app.status === 'ready' ? (agent?.label ?? 'Ready · no agent running') : app.status === 'loading' ? 'Connecting…' : 'Desktop runtime unavailable',
  );
  const tone = $derived(!connected || agent?.tone === 'warning' ? 'warning' : agent?.tone === 'danger' ? 'danger' : 'success');

  const status = $derived(git.status?.isRepo ? git.status : null);
  const themeIcon = $derived({ system: Monitor, dark: Moon, light: Sun }[prefs.value?.theme ?? 'system']);
  const themeName = $derived({ system: 'system', dark: 'dark', light: 'light' }[prefs.value?.theme ?? 'system']);
  const motionName = $derived(prefs.value?.motion ?? 'full');
  const updateLabel = $derived(indicatorLabel(updates.state, updates.fraction));
</script>

<!-- Live state only: what the agent is doing, the branch, a pending update, and the quick toggles. Which harness,
     project and app this is already shows in the top bar and in Settings, so the status bar does not repeat it. -->
<footer class="intro-enter intro-top flex h-12 shrink-0 items-center justify-between gap-3 border-t border-line bg-surface-1/70 px-2 text-sm text-fg-3" style:--intro-delay="140ms">
  <div class="flex min-w-0 items-center gap-1">
    <span class="sr-only" role="status">{label}</span>

    <StatusItem label="{label}. Open chat" onclick={() => nav.go('chat')}>
      {#snippet content()}
        <!-- The dot is alive: it pings like the dashboard's while ready or streaming, and stays still on a problem. -->
        <StatusDot {tone} live={tone === 'success'} />
        <span class={cx('truncate', tone === 'warning' && 'text-warning', tone === 'danger' && 'text-danger')}>{label}</span>
      {/snippet}
    </StatusItem>

    {#if status}
      <StatusItem label="Branch {status.branch ?? 'detached'}. Open changes" onclick={() => nav.go('changes')}>
        {#snippet content()}
          <GitBranch size={ICON.sm} aria-hidden="true" />
          <span class="max-w-40 truncate">{status.branch ?? 'detached HEAD'}</span>
          {#if status.ahead > 0}<span class="tabular inline-flex items-center text-success"><ArrowUp size={ICON.sm} aria-hidden="true" />{status.ahead}<span class="sr-only">{' ahead'}</span></span>{/if}
          {#if status.behind > 0}<span class="tabular inline-flex items-center text-warning"><ArrowDown size={ICON.sm} aria-hidden="true" />{status.behind}<span class="sr-only">{' behind'}</span></span>{/if}
        {/snippet}
      </StatusItem>
    {/if}
  </div>

  <div class="tabular flex items-center gap-1">
    {#if updateLabel}
      <StatusItem label="{updateLabel}. Open update settings" onclick={viewUpdates}>
        {#snippet content()}
          <Download size={ICON.sm} class="text-accent-text" aria-hidden="true" />
          <span>{updateLabel}</span>
        {/snippet}
      </StatusItem>

      <span class="mx-1 h-4 w-px bg-line-strong" aria-hidden="true"></span>
    {/if}

    <StatusItem label="Theme: {themeName}. Click to change" onclick={cycleTheme}>
      {#snippet content()}
        {@const Icon = themeIcon}
        <Icon size={ICON.sm} aria-hidden="true" />
      {/snippet}
    </StatusItem>

    <StatusItem label="Motion: {motionName}. Click to change" onclick={cycleMotion}>
      {#snippet content()}<Zap size={ICON.sm} aria-hidden="true" />{/snippet}
    </StatusItem>

    <StatusItem label="Keyboard shortcuts" shortcut="shift+?" onclick={onshortcuts}>
      {#snippet content()}<Keyboard size={ICON.sm} aria-hidden="true" />{/snippet}
    </StatusItem>
  </div>
</footer>
