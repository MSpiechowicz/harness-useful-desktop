<script lang="ts">
  import GitPullRequest from '@lucide/svelte/icons/git-pull-request';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeCompare } from '$lib/ipc/bindings/KnowledgeCompare';
  import type { ViewStatus } from '$lib/stores/view-loader.svelte';
  import { cx } from '$lib/utils/cx';

  import { changeItem, totalsOf } from './review-changes';

  interface Props {
    /** The pending version against its live document; null until it was read. */
    compare: KnowledgeCompare | null;
    status: ViewStatus;
    /** Opens the full review, at a section when one was chosen. */
    onreview: (section: string | null) => void;
  }

  let { compare, status, onreview }: Props = $props();

  /** How many rows the summary lists before it says "and N more". */
  const SHOWN = 4;

  const TONES = { success: 'bg-success-wash text-success', warning: 'bg-warning-wash text-warning', danger: 'bg-danger-wash text-danger', info: 'bg-info-wash text-info', neutral: 'bg-surface-3 text-fg-2' } as const;

  const sections = $derived(compare?.sections ?? []);
  const totals = $derived(totalsOf(sections));
  // A heading that repeats is told apart by its number, so each row has its own key and opens its own section.
  const items = $derived(sections.slice(0, SHOWN).map((diff) => changeItem(diff, 0)));
</script>

{#if compare === null}
  <p class="text-sm text-fg-3" role={status === 'error' ? 'alert' : 'status'}>{status === 'error' ? t('knowledge.drafts.changes.failed') : t('knowledge.drafts.changes.loading')}</p>
{:else}
  <!-- The summary reads as the Changes view lists files: a coloured letter per section, the counts in the diff's own colours. One button opens the whole review. -->
  <p class="mb-3 flex items-baseline gap-2 text-sm text-fg">
    <span class="tabular"><span class="text-success">+{totals.additions}</span> <span class="text-danger">−{totals.deletions}</span></span>
    <span>{tn('knowledge.drafts.changes.inSections', totals.sections)}</span>
    {#if compare.live == null}<span class="text-fg-3">· {t('knowledge.drafts.changes.newDocument')}</span>{/if}
  </p>

  <ul class="-mx-2 mb-1 flex flex-col gap-0.5">
    {#each items as item (item.id)}
      <li>
        <button type="button" class="flex min-h-9 w-full items-center gap-2.5 rounded-md px-2 py-1 text-left transition-colors duration-[var(--dur-fast)] hover:bg-hover active:bg-press" onclick={() => onreview(item.id)}>
          <span class={cx('grid size-5 shrink-0 place-items-center rounded-sm font-mono text-[11px] font-semibold', TONES[item.tone])} title={item.letterLabel}>{item.letter}<span class="sr-only">{` ${item.letterLabel}`}</span></span>
          <span class="min-w-0 flex-1 truncate text-sm text-fg" title={item.label}>{item.label}</span>
          <span class="tabular shrink-0 text-xs"><span class="text-success">+{item.additions}</span> <span class="text-danger">−{item.deletions}</span></span>
        </button>
      </li>
    {/each}
  </ul>

  {#if sections.length > SHOWN}
    <p class="mb-1 text-xs text-fg-3">{tn('knowledge.drafts.changes.more', sections.length - SHOWN)}</p>
  {/if}

  <div class="mt-3 flex items-center gap-2">
    <Button onclick={() => onreview(null)}>
      {#snippet leading()}<GitPullRequest size={ICON.sm} aria-hidden="true" />{/snippet}
      {t('knowledge.drafts.changes.review')}
    </Button>
  </div>
{/if}
