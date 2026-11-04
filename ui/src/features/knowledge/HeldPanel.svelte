<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import GitBranch from '@lucide/svelte/icons/git-branch';
  import X from '@lucide/svelte/icons/x';

  import Button from '$lib/components/Button.svelte';
  import FactsCard from '$lib/components/FactsCard.svelte';
  import { ICON } from '$lib/components/icon';
  import SectionCard from '$lib/components/SectionCard.svelte';
  import SidePanel from '$lib/components/SidePanel.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeHeldItem } from '$lib/ipc/bindings/KnowledgeHeldItem';
  import type { ViewStatus } from '$lib/stores/view-loader.svelte';

  import HiddenText from '../memory/HiddenText.svelte';
  import { revealHidden } from '../memory/script-format';
  import { formatLabel, heldFacts, linksText, repoLabel } from './knowledge-format';

  interface Props {
    open: boolean;
    /** The proposal as the list gave it (its text cut short), or as the read of it did. Null while one that is not on this page is being read. */
    item: KnowledgeHeldItem | null;
    /** The proposal with all of its text; null until it was read. */
    detail: KnowledgeHeldItem | null;
    status: ViewStatus;
    error: string | null;
    /** The proposal is being decided. */
    busy: boolean;
    onretry: () => void;
    onapprove: (item: KnowledgeHeldItem) => void;
    onreject: (item: KnowledgeHeldItem) => void;
    onclose: () => void;
  }

  let { open, item, detail, status, error, busy, onretry, onapprove, onreject, onclose }: Props = $props();

  // Whoever is decided on is the version that was shown: the read of it is newer than the row once it is in.
  const shown = $derived(detail ?? item);
  const text = $derived(shown?.text ?? '');
  // The list only gives a preview, so hidden characters are only looked for in the whole text, and a proposal is approved only after that was read.
  const hidden = $derived(revealHidden(detail?.text ?? '').count);
</script>

<SidePanel {open} title={item?.title ?? ''} subtitle={item ? `${formatLabel(item.format)} · ${linksText(item.links)}` : ''} {onclose}>
  {#if item && shown}
    <SectionCard flat heading={t('knowledge.held.content')}>
      {#if hidden > 0}
        <p class="mb-3 rounded-md border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger" role="alert">{tn('knowledge.held.hidden', hidden)}</p>
      {/if}

      <pre class="max-h-80 overflow-auto rounded-md border border-line bg-surface-2 p-3 font-mono text-xs leading-relaxed break-words whitespace-pre-wrap text-fg-2"><HiddenText {text} /></pre>

      {#if status === 'error'}
        <p class="mt-2 flex items-center gap-2 text-sm text-danger" role="alert">
          {error ?? t('knowledge.held.body.failed')}
          <Button variant="ghost" onclick={onretry}>{t('memory.tryAgain')}</Button>
        </p>
      {:else if detail === null}
        <p class="mt-2 text-xs text-fg-3" role="status">{t('knowledge.held.body.loading')}</p>
      {/if}
    </SectionCard>

    <SectionCard flat heading={t('knowledge.held.links')}>
      {#if shown.links.length === 0}
        <p class="text-sm text-fg-3">{t('knowledge.held.links.global')}</p>
      {:else}
        <ul class="flex flex-col gap-2">
          {#each shown.links as link, index (index)}
            <li class="flex flex-col gap-1.5 rounded-lg border border-line bg-surface-2 p-3">
              <span class="flex items-center gap-2.5 text-sm font-medium text-fg"><GitBranch size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" /><span class="truncate" title={link.repo}>{repoLabel(link.repo)}</span></span>

              {#if link.globs.length > 0}
                <ul class="flex flex-wrap gap-1.5">
                  {#each link.globs as glob, position (position)}
                    <li class="rounded-sm bg-surface-3 px-2 py-0.5 font-mono text-xs text-fg-2">{glob}</li>
                  {/each}
                </ul>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </SectionCard>

    <FactsCard flat heading={t('knowledge.panel.details')} rows={heldFacts(shown)} />

    <SectionCard flat heading={t('knowledge.held.decide')}>
      <p class="mb-3 text-sm text-fg-3">{t('knowledge.review.hint')}</p>

      <div class="flex flex-wrap items-center gap-2">
        <Button variant="primary" loading={busy} disabled={detail === null || status === 'error'} onclick={() => onapprove(detail ?? shown)}>
          {#snippet leading()}<Check size={ICON.sm} aria-hidden="true" />{/snippet}
          {t('knowledge.held.approve')}
        </Button>

        <Button variant="dangerQuiet" disabled={busy} onclick={() => onreject(shown)}>
          {#snippet leading()}<X size={ICON.sm} aria-hidden="true" />{/snippet}
          {t('knowledge.held.reject')}
        </Button>
      </div>
    </SectionCard>
  {/if}
</SidePanel>
