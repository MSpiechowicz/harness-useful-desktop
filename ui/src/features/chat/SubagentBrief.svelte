<script lang="ts">
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right';
  import Bot from '@lucide/svelte/icons/bot';

  import Button from '$lib/components/Button.svelte';
  import Markdown from '$lib/components/Markdown.svelte';
  import { timeOfDay } from '$lib/chat/subagent-time';
  import { subagentTitle } from '$lib/chat/subagents';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { Subagent } from '$lib/ipc/bindings/Subagent';

  import SubagentCardHeader from './SubagentCardHeader.svelte';

  interface Props {
    subagent: Subagent;
    /** When the subagent was spawned, on this computer's clock; null while unknown. */
    spawnedAt: number | null;
    /** The readable name of the tool call that spawned it (`Task`); null when that call is not in the transcript. */
    spawnTool: string | null;
    /** Whether that call can be scrolled to: it is in the transcript the list holds. */
    canJump: boolean;
    onjump: () => void;
  }

  let { subagent, spawnedAt, spawnTool, canJump, onjump }: Props = $props();

  const time = $derived(spawnedAt === null ? null : timeOfDay(spawnedAt));
</script>

<!-- The brief as the orchestrator wrote it. It stays in place above the messages sent later, which scroll under it; a very long
     brief scrolls inside the card instead of pushing them away. -->
<article class="overflow-hidden rounded-md border border-line bg-surface-2">
  <SubagentCardHeader copyText={subagent.task ?? ''} copyLabel={t('subagents.brief.copy')}>
    {#snippet icon()}<Bot size={16} aria-hidden="true" />{/snippet}
    {#snippet title()}<b class="text-sm font-medium text-fg">{t('subagents.brief.from')}</b>{/snippet}
    {#snippet sub()}{t('subagents.brief.to', { title: subagentTitle(subagent) })}{time ? ` · ${time}` : ''}{/snippet}
  </SubagentCardHeader>

  <div class="max-h-[var(--brief-max-h)] overflow-y-auto px-4 pt-3.5 pb-3 text-base">
    <Markdown source={subagent.task ?? ''} />
  </div>

  {#if spawnTool}
    <footer class="flex items-center gap-2 border-t border-line py-1.5 pr-1.5 pl-4 text-xs text-fg-3">
      <span class="min-w-0 flex-1">{time ? t('subagents.brief.spawnedAt', { tool: spawnTool, time }) : t('subagents.brief.spawned', { tool: spawnTool })}</span>
      {#if canJump}
        <Button variant="ghost" class="text-accent-text! hover:bg-accent-wash! hover:text-accent-text!" onclick={onjump}>
          {t('subagents.brief.jump')}
          {#snippet trailing()}<ArrowUpRight size={14} aria-hidden="true" />{/snippet}
        </Button>
      {/if}
    </footer>
  {/if}
</article>
