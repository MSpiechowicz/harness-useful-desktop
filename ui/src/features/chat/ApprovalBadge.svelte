<script lang="ts">
  import ShieldAlert from '@lucide/svelte/icons/shield-alert';
  import ShieldCheck from '@lucide/svelte/icons/shield-check';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import type { ApprovalMode } from '$lib/ipc/bindings/ApprovalMode';
  import { agentSessions } from '$lib/stores/agentSession.svelte';

  import { activeMode, MODE_ORDER, MODES } from './approval-modes';
  import MenuItem from './MenuItem.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  const CLOSE_MS = 220;

  /** `null` is "use the harness configuration". */
  let choice = $state<ApprovalMode | null>(null);
  let restarting = $state(false);

  const mode = $derived(activeMode(session.info.approvalOverride, session.info.configuredApprovalMode));
  const info = $derived(mode ? MODES[mode] : null);
  const source = $derived(session.info.approvalOverride ? 'chosen for this session' : 'from your harness configuration');
  const changed = $derived(choice !== session.info.approvalOverride);

  $effect(() => {
    if (session.ui.approvalChooser) {
      choice = session.info.approvalOverride;
    }
  });

  async function apply(): Promise<void> {
    restarting = true;

    // Let the popover finish closing before its session (and this badge) is replaced.
    session.ui.approvalChooser = false;
    await new Promise((resolve) => setTimeout(resolve, CLOSE_MS));

    await agentSessions.restart(session.key, { approvalOverride: choice });
  }
</script>

<Popover label="Approval mode" kind="dialog" bind:open={session.ui.approvalChooser} class="w-80">
  {#snippet trigger({ props })}
    <Tooltip text={info ? `${info.explanation} (${source})` : 'The approval mode is not known'} side="bottom">
      {#snippet children({ describedby })}
        <button type="button" {...props} aria-describedby={describedby} class="rounded-full" aria-label="Approval mode: {info?.label ?? 'unknown'}. Change it">
          <Badge tone={info?.tone ?? 'neutral'}>
            {#if info?.tone === 'warning'}<ShieldAlert size={12} aria-hidden="true" />{:else}<ShieldCheck size={12} aria-hidden="true" />{/if}
            Approvals: {info?.label ?? 'unknown'}
          </Badge>
        </button>
      {/snippet}
    </Tooltip>
  {/snippet}

  {#snippet children()}
    <div class="flex flex-col gap-3 p-1.5">
      <p class="text-sm text-fg-2">
        This session runs with <strong class="text-fg">{info?.label ?? 'an unknown mode'}</strong>: {info?.explanation ?? 'the mode could not be read'} ({source}).
      </p>

      <div class="flex flex-col" role="radiogroup" aria-label="Approval mode for this session">
        <MenuItem label="Use my configuration" detail={session.info.configuredApprovalMode ? MODES[session.info.configuredApprovalMode].label : null} selected={choice === null} onselect={() => (choice = null)} />
        {#each MODE_ORDER as option (option)}
          <MenuItem label={MODES[option].label} detail={MODES[option].tone === 'warning' ? 'no prompts' : null} selected={choice === option} onselect={() => (choice = option)} />
        {/each}
      </div>

      <p class="text-xs text-fg-3">A new mode applies when the agent starts, so the session restarts and picks up where it left off.</p>
      <Button variant="primary" size="sm" disabled={!changed} loading={restarting} onclick={apply}>Restart with this mode</Button>
    </div>
  {/snippet}
</Popover>
