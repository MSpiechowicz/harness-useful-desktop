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
  import Logo from '$lib/components/Logo.svelte';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { app } from '$lib/stores/app.svelte';
  import { git } from '$lib/stores/git.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { project } from '$lib/stores/project.svelte';
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
  // The mark blinks while any agent, not just the visible one, is working.
  const working = $derived(agentSessions.sessions.some((session) => session.chat.phase === 'streaming'));
  const label = $derived(
    app.status === 'ready' ? (agent?.label ?? 'Ready · no agent running') : app.status === 'loading' ? 'Connecting…' : 'Desktop runtime unavailable',
  );
  const tone = $derived(!connected || agent?.tone === 'warning' ? 'warning' : agent?.tone === 'danger' ? 'danger' : 'ok');

  const status = $derived(git.status?.isRepo ? git.status : null);
  const themeIcon = $derived({ system: Monitor, dark: Moon, light: Sun }[prefs.value?.theme ?? 'system']);
  const themeName = $derived({ system: 'system', dark: 'dark', light: 'light' }[prefs.value?.theme ?? 'system']);
  const motionName = $derived(prefs.value?.motion ?? 'full');
  const updateLabel = $derived(indicatorLabel(updates.state, updates.fraction));
</script>

<footer class="intro-enter intro-top flex h-8 shrink-0 items-center justify-between gap-3 border-t border-line bg-surface-1/70 px-2 text-xs text-fg-3" style:--intro-delay="140ms">
  <div class="flex min-w-0 items-center gap-1">
    <span class="sr-only" role="status">{label}</span>

    <StatusItem label="{label}. Open chat" onclick={() => nav.go('chat')}>
      {#snippet content()}
        <span class="grid size-4 place-items-center" aria-hidden="true">
          <Logo size={14} state={working ? 'working' : 'idle'} />
        </span>
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
    {/if}

    {#if harness.active}
      <span class="max-[1100px]:hidden">
        <StatusItem label="{harness.active.info.name} configuration" onclick={() => nav.go('config')}>
          {#snippet content()}
            {harness.active?.info.shortName}{harness.active?.detection.version ? ` v${harness.active.detection.version}` : ''}
          {/snippet}
        </StatusItem>
      </span>
    {/if}

    <span class="max-[900px]:hidden">
      <StatusItem label="{project.active ? project.active.name : 'No project'}. Browse sessions" onclick={() => nav.go('sessions')}>
        {#snippet content()}{project.active ? project.active.name : 'No project'}{/snippet}
      </StatusItem>
    </span>

    {#if app.info}
      <span class="max-[1100px]:hidden">
        <StatusItem label="{app.info.name} v{app.info.version}. Open settings" onclick={() => nav.go('settings')}>
          {#snippet content()}{app.info?.name} v{app.info?.version}{/snippet}
        </StatusItem>
      </span>
    {/if}

    <span class="mx-1 h-4 w-px bg-line-strong" aria-hidden="true"></span>

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
