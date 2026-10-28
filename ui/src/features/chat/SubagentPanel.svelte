<script lang="ts">
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import SidePanel from '$lib/components/SidePanel.svelte';
  import type { StripOption } from '$lib/components/strip';
  import { currentAction, indexOfToolCall, latestOutput, sentMessages, toolStepCount } from '$lib/chat/subagent-steps';
  import { elapsedMs, messageDelivery, subagentTitle } from '$lib/chat/subagents';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { own } from '$lib/chat/state';
  import { cx } from '$lib/utils/cx';
  import { humanToolName } from '$lib/chat/tool-meta';
  import { panelScroller } from '$lib/chat/subagent-follow';
  import { t } from '$lib/i18n/i18n.svelte';
  import { subagentPanel, type PanelTab } from '$lib/stores/subagentPanel.svelte';

  import SubagentActivity from './SubagentActivity.svelte';
  import SubagentStats from './SubagentStats.svelte';
  import SubagentStepRow from './SubagentStepRow.svelte';
  import SubagentBrief from './SubagentBrief.svelte';
  import SubagentLiveCard from './SubagentLiveCard.svelte';
  import SubagentMessageBox from './SubagentMessageBox.svelte';
  import SubagentOutput from './SubagentOutput.svelte';

  interface Props {
    session: ChatSession;
    /** The chat pane the panel slides over: its bottom edge and the composer card inside it set the height of the message area. */
    host: HTMLElement | undefined;
  }

  let { session, host }: Props = $props();

  const TICK_MS = 1000;

  const key = $derived(session.key);
  const chat = $derived(session.chat);
  const selected = $derived(subagentPanel.selectedId(key));

  // The panel slides out after it closes, so it keeps showing the subagent it showed until it is gone.
  let shown = $state<string | null>(null);

  $effect.pre(() => {
    if (selected !== null) {
      shown = selected;
    }
  });

  const subagent = $derived(shown === null ? undefined : own(chat.subagents, shown));
  const open = $derived(selected !== null && subagent !== undefined);
  const entries = $derived(shown === null ? [] : (own(chat.subagentSteps, shown) ?? []));
  const clock = $derived(shown === null ? undefined : own(chat.subagentClock, shown));
  const outputStep = $derived(latestOutput(entries));

  // A subagent that is gone (the chat started over) takes its panel with it.
  $effect(() => {
    if (selected !== null && own(chat.subagents, selected) === undefined) {
      subagentPanel.close(key);
    }
  });

  // Tabs without data are not offered: the assignment is the Brief, the end of a result or the final report is the Output.
  const tabs = $derived.by((): StripOption<PanelTab>[] => {
    const list: StripOption<PanelTab>[] = [{ value: 'activity', label: t('subagents.tab.activity') }];

    if (subagent?.task) {
      list.push({ value: 'brief', label: t('subagents.tab.brief') });
    }
    if (subagent?.report || outputStep) {
      list.push({ value: 'output', label: subagent?.report ? t('subagents.tab.result') : t('subagents.tab.output') });
    }

    return list;
  });
  const stored = $derived(subagentPanel.tab(key));
  const tab = $derived(tabs.some((option) => option.value === stored) ? stored : 'activity');

  // Another tab or another subagent starts at the top; the steps list follows its own end (see `SubagentActivity`).
  let content: HTMLElement | undefined = $state();

  $effect(() => {
    void [tab, shown];
    const scroller = panelScroller(content);

    if (scroller && tab !== 'activity') {
      scroller.scrollTop = 0;
    }
  });

  let now = $state(Date.now());
  const running = $derived(subagent?.status === 'running');

  // The timer ticks once a second, and only while the panel is open on a subagent that runs.
  $effect(() => {
    if (!open || !running) {
      return;
    }

    now = Date.now();
    const timer = setInterval(() => (now = Date.now()), TICK_MS);

    return () => clearInterval(timer);
  });

  const ms = $derived(subagent ? elapsedMs(subagent, clock, now) : null);
  const subtitle = $derived(subagent ? [subagent.agent, subagent.model, subagent.detached ? t('subagents.background') : null].filter(Boolean).join(' · ') : '');
  const delivery = $derived(subagent ? messageDelivery(subagent) : null);
  const later = $derived(sentMessages(entries));

  const placeholder = $derived(delivery === 'direct' && subagent ? t('subagents.message.placeholderDirect', { name: subagentTitle(subagent) }) : t('subagents.message.placeholderOrchestrator'));

  // The message area's top edge is the composer card's top edge, whatever height the composer has.
  let zone = $state<number | null>(null);
  // The composer card's own height: the message box grows to end where the card ends.
  let cardHeight = $state<number | null>(null);

  $effect(() => {
    if (!open || !host || typeof ResizeObserver === 'undefined') {
      return;
    }

    const pane = host;
    const card = pane.querySelector<HTMLElement>('[data-composer-card]');
    if (!card) {
      return;
    }

    let frame = 0;

    // At most once a frame, and the card is looked up again: the composer may have put a new one in place.
    const measure = (): void => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const current = pane.querySelector<HTMLElement>('[data-composer-card]') ?? card;
        const box = current.getBoundingClientRect();

        zone = Math.round(pane.getBoundingClientRect().bottom - box.top);
        cardHeight = Math.round(box.height);
      });
    };
    const observer = new ResizeObserver(measure);

    // The pane, the card and every block of the pane are watched: whatever sits below the card (a widget, the
    // footer) moves its top edge when it grows. Watching an element twice is a no-op, so this runs again for blocks
    // that mount later and for a card the composer put in place.
    const watch = (): void => {
      const current = pane.querySelector<HTMLElement>('[data-composer-card]') ?? card;

      for (const element of [pane, current, current.parentElement, ...pane.children]) {
        if (element) {
          observer.observe(element);
        }
      }
    };

    // A block mounting or leaving above or below the card (a widget, a banner, the docked subagents) moves it
    // without resizing anything already watched. Those are the pane's own children, so the streamed transcript
    // inside the message list does not wake it.
    const mutations = new MutationObserver(() => {
      watch();
      measure();
    });

    watch();
    measure();
    mutations.observe(pane, { childList: true });

    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      mutations.disconnect();
    };
  });

  let stopping = $state(false);

  async function stop(): Promise<void> {
    if (!subagent) {
      return;
    }

    stopping = true;
    await session.stopSubagent(subagent.id);
    stopping = false;
  }

  async function send(text: string): Promise<boolean> {
    if (!subagent) {
      return false;
    }

    const sent = await session.messageSubagent(subagent, text);
    if (sent) {
      subagentPanel.setTab(key, 'activity');
    }

    return sent;
  }

  /** Closes the panel; a person typing in its message box gets the chat's composer back. */
  function close(): void {
    const typing = document.activeElement?.closest('[data-subagent-message]') != null;

    subagentPanel.close(key);
    if (typing) {
      host?.querySelector<HTMLElement>('[data-composer-card] textarea')?.focus();
    }
  }

  // Escape closes the panel unless something else took it (a menu, a dialog). The composer's double Escape that stops the whole agent
  // looks at the same panel and stays out of it, so closing the panel never counts toward stopping the agent.
  function onkeydown(event: KeyboardEvent): void {
    if (event.key !== 'Escape' || event.defaultPrevented || event.isComposing || !open || document.querySelector('[role="dialog"][aria-modal="true"]')) {
      return;
    }

    event.preventDefault();
    close();
  }
</script>

<svelte:window {onkeydown} />

<SidePanel flat {open} title={subagent ? subagentTitle(subagent) : ''} {subtitle} onclose={close}>
  {#snippet top()}
    {#if subagent}
      <!-- One card per subagent: what it shows (the check that draws once, the line that rolls) belongs to the one on screen. -->
      {#key subagent.id}
        <SubagentLiveCard {subagent} action={currentAction(entries, subagent)} {ms} toolCalls={subagent.toolCount ?? toolStepCount(entries)} {stopping} onstop={() => void stop()} />
      {/key}

      {#if tabs.length > 1}
        <div class="panel-pad pt-[var(--panel-block-gap)]">
          <SegmentedControl options={tabs} value={tab} label={t('subagents.tabs.label')} equal class="w-full" onchange={(next) => subagentPanel.setTab(key, next)} />
        </div>
      {/if}

      <!-- What a tab is about stays put, only its list scrolls: the figures over the steps, the brief over the messages sent later. -->
      {#if tab === 'activity'}
        <div class="panel-pad pt-[var(--panel-block-gap)]">
          <SubagentStats {subagent} {entries} />
        </div>
      {:else if tab === 'brief'}
        {@const call = subagent.parentToolCallId ? own(chat.toolCalls, subagent.parentToolCallId) : undefined}
        <div class="panel-pad pt-[var(--panel-block-gap)]">
          <SubagentBrief
            {subagent}
            spawnedAt={clock?.startedAt ?? null}
            spawnTool={call ? humanToolName(call.name) : null}
            canJump={call !== undefined && indexOfToolCall(chat.items, call.id) !== -1}
            onjump={() => call && subagentPanel.jumpTo(key, call.id, subagent.id)}
          />
          {#if later.length > 0}
            <h3 class="mt-4 text-xs tracking-wide text-fg-3 uppercase">{t('subagents.brief.later')}</h3>
          {/if}
        </div>
      {/if}
    {/if}
  {/snippet}

  {#if subagent}
    <!-- On the output the content is as tall as the body: the card fits in it, its header stays and only its middle scrolls. The
         steps and the messages sent later scroll with the body. -->
    <div bind:this={content} class={cx('panel-pad pt-[var(--panel-block-gap)] pb-[var(--panel-fade-end)]', tab === 'output' && 'flex min-h-0 flex-1 flex-col')}>
      {#if tab === 'brief'}
        <ul class="m-0 list-none p-0" aria-label={t('subagents.brief.later')}>
          {#each later as entry (entry.id)}
            <SubagentStepRow {entry} startedAt={clock?.startedAt ?? null} />
          {/each}
        </ul>
      {:else if tab === 'output'}
        <SubagentOutput {subagent} step={outputStep} {ms} />
      {:else}
        <SubagentActivity {subagent} {entries} startedAt={clock?.startedAt ?? null} />
      {/if}
    </div>
  {/if}

  {#snippet footer()}
    {#if subagent && delivery !== null}
      {#key subagent.id}
        <SubagentMessageBox {placeholder} disabled={session.ended} height={zone} {cardHeight} onsend={send} />
      {/key}
    {/if}
  {/snippet}
</SidePanel>
