<script lang="ts">
  import ShieldAlert from '@lucide/svelte/icons/shield-alert';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import ShieldCheck from '@lucide/svelte/icons/shield-check';
  import ShieldHalf from '@lucide/svelte/icons/shield-half';
  import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import type { ApprovalMode } from '$lib/ipc/bindings/ApprovalMode';
  import { agentSessions } from '$lib/stores/agentSession.svelte';

  import { activeMode, MODE_ORDER, MODES } from './approval-modes';
  import ApprovalOption from './ApprovalOption.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  const CLOSE_MS = 220;
  const MODE_ICONS = { alwaysAsk: ShieldCheck, write: ShieldHalf, yolo: ShieldAlert } satisfies Record<ApprovalMode, unknown>;

  /** `null` is "use the harness configuration". */
  let choice = $state<ApprovalMode | null>(null);
  let restarting = $state(false);

  const mode = $derived(activeMode(session.info.approvalOverride, session.info.configuredApprovalMode));
  const info = $derived(mode ? MODES[mode] : null);
  const source = $derived(session.info.approvalOverride ? 'chosen for this session' : 'from your harness configuration');
  const configured = $derived(session.info.configuredApprovalMode);
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

<Popover label="Approval mode" kind="dialog" bind:open={session.ui.approvalChooser} class="w-[22rem]">
  {#snippet trigger({ props })}
    <Tooltip text={info ? `${info.explanation} (${source})` : 'The approval mode is not known'} side="bottom">
      {#snippet children({ describedby })}
        <button type="button" {...props} aria-describedby={describedby} class="rounded-full" aria-label="Approval mode: {info?.label ?? 'unknown'}. Change it">
          <Badge tone={info?.tone ?? 'neutral'}>
            {#if info?.tone === 'warning'}<ShieldAlert size={16} aria-hidden="true" />{:else}<ShieldCheck size={16} aria-hidden="true" />{/if}
            Approvals: {info?.label ?? 'unknown'}
          </Badge>
        </button>
      {/snippet}
    </Tooltip>
  {/snippet}

  {#snippet children()}
    <div class="flex flex-col">
      <header class="flex flex-col gap-0.5 px-2.5 pt-2 pb-3">
        <h2 class="text-sm font-semibold text-fg">Approvals</h2>
        <p class="text-xs text-fg-3">When the agent asks before it runs a tool.</p>
      </header>

      <div class="flex flex-col gap-0.5" role="radiogroup" aria-label="Approval mode for this session">
        <ApprovalOption
          label="Use my configuration"
          description={configured ? `${MODES[configured].label}, from your harness settings` : 'From your harness settings'}
          icon={SlidersHorizontal}
          tone="neutral"
          selected={choice === null}
          current={session.info.approvalOverride === null}
          onselect={() => (choice = null)}
        />
        {#each MODE_ORDER as option (option)}
          <ApprovalOption
            label={MODES[option].label}
            description={MODES[option].explanation}
            icon={MODE_ICONS[option]}
            tone={MODES[option].tone}
            selected={choice === option}
            current={session.info.approvalOverride === option}
            onselect={() => (choice = option)}
          />
        {/each}
      </div>

      <footer class="mt-1.5 flex flex-col gap-4 border-t border-line px-2.5 pt-3 pb-1.5">
        <p class="flex items-start gap-1.5 text-xs text-fg-3">
          <RotateCcw size={14} aria-hidden="true" class="mt-px shrink-0" />
          The session restarts to apply it and picks up where it left off.
        </p>
        <Button variant="primary" size="sm" disabled={!changed} loading={restarting} onclick={apply}>Restart with this mode</Button>
      </footer>
    </div>
  {/snippet}
</Popover>
