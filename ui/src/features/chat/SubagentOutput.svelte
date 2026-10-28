<script lang="ts">
  import Check from '@lucide/svelte/icons/check';

  import Button from '$lib/components/Button.svelte';
  import Markdown from '$lib/components/Markdown.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { nearEnd } from '$lib/chat/subagent-follow';
  import { outputView } from '$lib/chat/subagent-steps';
  import { elapsed } from '$lib/chat/subagents';
  import { humanToolName, TOOL_ICONS, toolKind } from '$lib/chat/tool-meta';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { Subagent } from '$lib/ipc/bindings/Subagent';
  import type { SubagentStep } from '$lib/ipc/bindings/SubagentStep';
  import { cx } from '$lib/utils/cx';

  import SubagentCardHeader from './SubagentCardHeader.svelte';

  interface Props {
    subagent: Subagent;
    /** The newest step that kept the end of its result; null when none did. */
    step: SubagentStep | null;
    /** How long the subagent ran, for the final report; null while unknown. */
    ms: number | null;
  }

  let { subagent, step, ms }: Props = $props();

  let showAll = $state(false);
  let body: HTMLElement | undefined = $state();
  let stuck = true;

  const running = $derived(subagent.status === 'running');
  const view = $derived(step ? outputView(step, showAll) : null);
  const Icon = $derived(TOOL_ICONS[toolKind(step?.name ?? '')]);
  // The line numbers take as many digits as the last one has, and no more.
  const gutter = $derived(view ? `calc(${String(view.first + view.lines.length - 1).length}ch + var(--output-gutter-pad))` : undefined);

  // The terminal follows its end while the person has not scrolled up.
  $effect.pre(() => {
    void view?.lines.length;

    if (body) {
      stuck = nearEnd(body);
    }
  });

  $effect(() => {
    void view?.lines.length;

    if (body && stuck) {
      body.scrollTop = body.scrollHeight;
    }
  });
</script>

{#snippet command()}<span class="block truncate font-mono" title={step?.detail}>{step?.detail}</span>{/snippet}

{#if subagent.report}
  <article class="flex min-h-0 flex-col overflow-hidden rounded-md border border-line bg-surface-2">
    <SubagentCardHeader tone="success" copyText={subagent.report} copyLabel={t('subagents.report.copy')}>
      {#snippet icon()}<Check size={16} aria-hidden="true" />{/snippet}
      {#snippet title()}<b class="text-sm font-medium text-fg">{t('subagents.report.title')}</b>{/snippet}
      {#snippet sub()}{t('subagents.report.sub')}{ms !== null ? ` · ${elapsed(ms)}` : ''}{/snippet}
    </SubagentCardHeader>

    <div class="min-h-0 flex-1 overflow-y-auto px-4 pt-3.5 pb-3 text-base">
      <Markdown source={subagent.report} />
    </div>
  </article>
{:else if step && view}
  <article class={cx('flex min-h-0 flex-col overflow-hidden rounded-md border bg-surface-2', running ? 'border-accent/30' : 'border-line')}>
    <!-- The tool and where it is on the first line, what it ran under it when the harness said. -->
    <SubagentCardHeader copyText={step.output ?? ''} copyLabel={t('subagents.output.copy')} sub={step.detail ? command : undefined}>
      {#snippet icon()}<Icon size={16} aria-hidden="true" />{/snippet}
      {#snippet title()}
        <b class="text-sm font-medium text-fg">{humanToolName(step.name ?? '')}</b>
        {#if running}<Spinner size={12} class="text-accent-text" />{t('subagents.output.live')}{:else}{t('subagents.output.finished')}{/if}
      {/snippet}
    </SubagentCardHeader>

    <!-- svelte-ignore a11y_no_noninteractive_tabindex -- a scrollable region must be reachable by keyboard -->
    <div bind:this={body} class="min-h-0 flex-1 overflow-auto bg-canvas py-2.5" tabindex="0" role="region" aria-label={t('subagents.tab.output')}>
      <ol start={view.first} class="m-0 list-decimal pr-3.5 font-mono text-xs leading-[var(--detail-line)] text-fg-2" style:padding-left={gutter}>
        {#each view.lines as line, index (view.first + index)}
          <li class={cx('pl-1 break-all whitespace-pre-wrap marker:text-fg-3', index === view.lines.length - 1 && 'text-fg')}>{line || ' '}{#if running && index === view.lines.length - 1}<span class="caret" aria-hidden="true"></span>{/if}</li>
        {/each}
      </ol>
    </div>

    <footer class="flex shrink-0 items-center gap-2 border-t border-line py-1.5 pr-1.5 pl-4 text-xs text-fg-3">
      <span class="min-w-0 flex-1">{view.lines.length < view.total ? t('subagents.output.last', { shown: view.lines.length, total: view.total }) : t('subagents.output.all', { total: view.total })}</span>
      {#if view.canExpand}
        <Button variant="ghost" class="text-accent-text! hover:bg-accent-wash! hover:text-accent-text!" onclick={() => (showAll = !showAll)}>
          {showAll ? t('subagents.output.showLast') : t('subagents.output.showAll')}
        </Button>
      {/if}
    </footer>
  </article>
{/if}
