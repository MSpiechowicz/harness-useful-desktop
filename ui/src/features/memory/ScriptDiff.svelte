<script lang="ts">
  import { DIFF_ROW, DIFF_TEXT } from '$lib/components/diff-tones';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  import HiddenText from './HiddenText.svelte';
  import type { LineDiff } from './line-diff';

  interface Props {
    /** What changed between the code the user approved and the code the agent asks to run. */
    diff: LineDiff;
  }

  let { diff }: Props = $props();

  const SIGNS = { add: '+', remove: '−', same: ' ' } as const;
  const TONES = { add: 'add', remove: 'remove', same: 'context' } as const;
</script>

<figure class="chat-radius overflow-hidden border border-line bg-canvas" aria-label={t('memory.request.diff.label')}>
  <!-- Like the code block, the diff shows in full: the dialog around it is the one thing that scrolls. -->
  <pre class="py-2 font-mono text-[13px] leading-5 whitespace-pre-wrap [overflow-wrap:anywhere]"><code class="grid">{#each diff.rows as row, index (index)}{#if row.kind === 'gap'}<span class="px-3.5 py-1 font-sans text-xs text-fg-3 select-none">{tn('memory.request.diff.gap', row.hidden)}</span>{:else}<span class={cx('grid grid-cols-[1.5ch_1fr] gap-x-2 px-3.5', DIFF_ROW[TONES[row.kind]], DIFF_TEXT[TONES[row.kind]])} data-diff={row.kind}><span class="select-none">{SIGNS[row.kind]}</span><span><HiddenText text={row.text} /></span></span>{/if}{/each}</code></pre>
</figure>
