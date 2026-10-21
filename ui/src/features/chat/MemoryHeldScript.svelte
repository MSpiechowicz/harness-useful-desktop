<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import { isLong, type HeldScriptView } from '$lib/chat/memory-held';
  import { t } from '$lib/i18n/i18n.svelte';

  import { lineDiff } from '../memory/line-diff';
  import ScriptCode from '../memory/ScriptCode.svelte';
  import ScriptDiff from '../memory/ScriptDiff.svelte';
  import { languageLabel, spelledOut } from '../memory/script-format';
  import MemoryFolded from './MemoryFolded.svelte';

  interface Props {
    view: HeldScriptView;
  }

  let { view }: Props = $props();

  type Shown = 'code' | 'changes';

  let shown = $state<Shown>('code');

  const script = $derived(view.script);
  const current = $derived(view.current);
  const diff = $derived(current ? lineDiff(current.body, script.body) : null);
  const VIEWS: StripOption<Shown>[] = [
    {
      value: 'code',
      get label() {
        return t('memory.request.view.code');
      },
    },
    {
      value: 'changes',
      get label() {
        return t('memory.request.view.changes');
      },
    },
  ];
</script>

<div class="flex flex-wrap items-center gap-2">
  <Badge size="lg">{t('memory.guards.waiting.script')}</Badge>
  <Badge size="lg">{languageLabel(script.language)}</Badge>
  <Badge size="lg">{script.scope === 'repo' ? t('memory.guards.waiting.scope.repo') : t('memory.guards.waiting.scope.global')}</Badge>
  <Badge size="lg" tone={current ? 'info' : 'neutral'}>{current ? t('memory.guards.waiting.script.replaces', { name: spelledOut(current.name) }) : t('memory.guards.waiting.script.new')}</Badge>
</div>

<h4 class="font-mono text-base font-semibold break-words text-fg">{spelledOut(script.name)}</h4>
<p class="text-sm break-words text-fg-2">{spelledOut(script.description)}</p>

{#if diff && current}
  <div class="flex">
    <SegmentedControl label={t('memory.request.view')} options={VIEWS} bind:value={shown} />
  </div>
{/if}

{#if shown === 'changes' && diff}
  {#if diff.rows.length > 0}
    <ScriptDiff {diff} label={t('memory.guards.waiting.script.diffLabel')} />
  {:else}
    <p class="text-sm text-fg-2">{t('memory.request.diff.same')}</p>
  {/if}
{:else}
  <MemoryFolded long={isLong(script.body)}>
    <ScriptCode body={script.body} language={script.language} />
  </MemoryFolded>
{/if}

<dl class="grid grid-cols-[auto_1fr] items-baseline gap-x-4 gap-y-2 text-sm text-fg-3">
  <dt>{t('memory.guards.waiting.script.runs')}</dt>
  <dd class="text-fg-2">{t('memory.guards.waiting.script.trust')}</dd>
  <dt>{t('memory.guards.waiting.script.limit')}</dt>
  <dd class="text-fg-2">{t('memory.guards.waiting.script.timeout', { seconds: script.timeoutSecs })}</dd>
</dl>
