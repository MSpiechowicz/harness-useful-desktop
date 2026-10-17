<script lang="ts">
  import Disclosure from '$lib/components/Disclosure.svelte';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { ScriptRunRequest } from '$lib/ipc/bindings/ScriptRunRequest';
  import { formatDateTime } from '$lib/utils/format';

  import HiddenText from './HiddenText.svelte';
  import { lineDiff } from './line-diff';
  import ScriptCode from './ScriptCode.svelte';
  import ScriptDiff from './ScriptDiff.svelte';
  import { authorText, languageLabel, PLACE_LABELS, revealHidden, scopeText, shellQuote, spelledOut } from './script-format';

  interface Props {
    request: ScriptRunRequest;
  }

  let { request }: Props = $props();

  type View = 'changes' | 'code';

  // A request is shown once, so the view it starts in is taken when it appears. The dialog shows the next request in a new body.
  // svelte-ignore state_referenced_locally
  let view = $state<View>(request.previous ? 'changes' : 'code');

  let stdinOpen = $state(false);

  const VIEWS: StripOption<View>[] = [
    {
      value: 'changes',
      get label() {
        return t('memory.request.view.changes');
      },
    },
    {
      value: 'code',
      get label() {
        return t('memory.request.view.code');
      },
    },
  ];

  const previous = $derived(request.previous);
  const diff = $derived(previous ? lineDiff(previous.body, request.body) : null);
  const hiddenCount = $derived(revealHidden(request.body).count);
  const argumentLine = $derived(request.args.map(shellQuote).join(' '));
  const approvedOn = $derived(previous ? formatDateTime(previous.trustedAt) || previous.trustedAt : '');
  // The client and the author name themselves, so what they wrote is shown with invisible characters spelled out and the label says whose words these are.
  const changedLabel = $derived(previous?.origin === 'saved' ? t('memory.request.changed.saved', { date: approvedOn }) : t('memory.request.changed', { date: approvedOn }));

  // What the server says the script was called and kept for when it was approved: a rename or a wider scope is a reason to read again.
  const renamedFrom = $derived(previous?.name && previous.name !== request.script.name ? previous.name : null);
  const scopeChange = $derived(previous?.scope && previous.scope !== request.script.scope ? { before: scopeText({ scope: previous.scope, repo: null }), after: scopeText(request.script) } : null);

  const facts = $derived([
    { label: t('memory.request.client'), value: request.client || t('memory.request.unknownClient'), claimed: request.client !== '' },
    { label: t('memory.request.author'), value: authorText(request.script), claimed: true },
    { label: t('memory.request.place'), value: t(PLACE_LABELS[request.script.place]), claimed: false },
  ]);
</script>

<div class="flex flex-col gap-4">
  <dl class="grid grid-cols-3 gap-x-4">
    {#each facts as fact (fact.label)}
      <div class="flex min-w-0 flex-col gap-0.5">
        <dt class="text-xs text-fg-3">{fact.label}</dt>
        <dd class="truncate text-sm text-fg" title={spelledOut(fact.value)}>{#if fact.claimed}<HiddenText text={fact.value} />{:else}{fact.value}{/if}</dd>
      </div>
    {/each}
  </dl>

  <div class="flex flex-col gap-1">
    <p class="text-xs text-fg-3">{t('memory.request.args')}</p>
    <p class="min-h-9 rounded-md border border-line bg-canvas px-3 py-2 font-mono text-[13px] leading-5 break-all whitespace-pre-wrap text-fg">
      {#if request.args.length > 0}<HiddenText text={argumentLine} />{:else}<span class="font-sans text-sm text-fg-3">{t('memory.request.noArgs')}</span>{/if}
    </p>
  </div>

  {#if request.cwd}
    <div class="flex flex-col gap-1">
      <p class="text-xs text-fg-3">{t('memory.request.cwd')}</p>
      <p class="rounded-md border border-line bg-canvas px-3 py-2 font-mono text-[13px] leading-5 break-all whitespace-pre-wrap text-fg"><HiddenText text={request.cwd} /></p>
    </div>
  {/if}

  {#if request.stdin}
    <Disclosure label={tn('memory.request.stdin', request.stdin.bytes)} bind:open={stdinOpen}>
      <pre class="max-h-40 overflow-auto rounded-md border border-line bg-canvas px-3 py-2 font-mono text-[13px] leading-5 break-all whitespace-pre-wrap text-fg"><HiddenText text={request.stdin.preview} /></pre>
    </Disclosure>
  {/if}

  <!-- The same two lines whether the script is new here or changed: what to look at, and a hint how much there is. -->
  <div class="flex items-center justify-between gap-3">
    <div class="flex min-w-0 flex-col gap-0.5">
      {#if previous && diff}
        <p class="text-sm font-medium text-fg">{changedLabel}</p>
        <p class="tabular font-mono text-xs">
          <span class="text-[color:var(--shiki-token-inserted)]">+{diff.added}</span>
          <span class="text-[color:var(--shiki-token-deleted)]">−{diff.removed}</span>
        </p>
      {:else}
        <p class="text-sm font-medium text-fg">{t('memory.request.first.title')}</p>
        <p class="text-xs text-fg-3">{t('memory.request.first.body')}</p>
      {/if}
    </div>

    {#if previous}
      <SegmentedControl label={t('memory.request.view')} options={VIEWS} bind:value={view} />
    {/if}
  </div>

  {#if renamedFrom !== null || scopeChange !== null}
    <div class="flex flex-col gap-1 text-sm text-warning">
      {#if renamedFrom !== null}
        <p>{t('memory.request.renamed', { name: spelledOut(renamedFrom) })}</p>
      {/if}
      {#if scopeChange !== null}
        <p>{t('memory.request.scopeChanged', scopeChange)}</p>
      {/if}
    </div>
  {/if}

  {#if view === 'changes' && previous && diff}
    {#if hiddenCount > 0}
      <p class="rounded-md border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger" role="alert">{tn('memory.scripts.body.hidden', hiddenCount)}</p>
    {/if}

    {#if previous.language !== request.language}
      <p class="text-sm text-warning">{t('memory.request.diff.interpreter', { before: languageLabel(previous.language), after: languageLabel(request.language) })}</p>
    {/if}

    {#if diff.rows.length > 0}
      <ScriptDiff {diff} />
    {:else}
      <p class="text-sm text-fg-2">{t('memory.request.diff.same')}</p>
    {/if}
  {:else}
    <ScriptCode body={request.body} language={request.language} />
  {/if}
</div>
