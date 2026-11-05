<script lang="ts">
  import GitBranch from '@lucide/svelte/icons/git-branch';
  import MessageSquareWarning from '@lucide/svelte/icons/message-square-warning';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import FactsCard from '$lib/components/FactsCard.svelte';
  import { ICON } from '$lib/components/icon';
  import LoadingSpinner from '$lib/components/LoadingSpinner.svelte';
  import SectionCard from '$lib/components/SectionCard.svelte';
  import SidePanel from '$lib/components/SidePanel.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeHeldItem } from '$lib/ipc/bindings/KnowledgeHeldItem';
  import type { ViewStatus } from '$lib/stores/view-loader.svelte';

  import HiddenText from '../memory/HiddenText.svelte';
  import { revealHidden } from '../memory/script-format';
  import { COMMENT_TEXT } from './comment-text';
  import HeldDecide from './HeldDecide.svelte';
  import { heldFacts, heldSubtitle, pathsBadge, repoLabel } from './knowledge-format';
  import { useReveal } from './panel-reveal.svelte';

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
    /** The chat of the agent that proposed it is open in this app, so a request for changes can reach it. */
    followUp: boolean;
    onretry: () => void;
    onapprove: (item: KnowledgeHeldItem) => void;
    onrequest: (item: KnowledgeHeldItem, comment: string) => void;
    onreject: (item: KnowledgeHeldItem, reason: string | null) => void;
    onclose: () => void;
  }

  let { open, item, detail, status, error, busy, followUp, onretry, onapprove, onrequest, onreject, onclose }: Props = $props();

  // The body waits for the whole text of the proposal behind one spinner, as the document panel does.
  const reveal = useReveal(() => (open && item ? item.id : null), () => detail !== null || status === 'error');

  // Whoever is decided on is the version that was shown: the read of it is newer than the row once it is in.
  const shown = $derived(detail ?? item);
  const text = $derived(shown?.text ?? '');
  // The list only gives a preview, so hidden characters are only looked for in the whole text, and a proposal is approved only after that was read.
  const hidden = $derived(revealHidden(detail?.text ?? '').count);
</script>

{#snippet statusRow()}
  {#if item}
    <Badge tone="danger" size="lg" class="shrink-0"><MessageSquareWarning size={ICON.sm} aria-hidden="true" />{t('knowledge.status.changesRequested')}</Badge>
    <span class="min-w-0 truncate text-xs text-fg-3" title={heldSubtitle(item)}>{heldSubtitle(item)}</span>
  {/if}
{/snippet}

<SidePanel {open} title={item?.title ?? ''} subtitle={item ? heldSubtitle(item) : ''} status={item?.status === 'changesRequested' ? statusRow : undefined} {onclose}>
  {#if item && !reveal.shown}
    <!-- `useReveal` already holds this spinner for its least time, so it needs no delay of its own. -->
    <LoadingSpinner label={t('knowledge.held.body.loading')} delay={0} class="flex-1 py-16" />
  {:else if item && shown}
    <SectionCard flat heading={t('knowledge.held.content')} hint={t('knowledge.held.content.hint')}>
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

    <!-- Every section says what it is under its heading. Without links, the hint says what that means. -->
    <SectionCard flat heading={t('knowledge.held.links')} hint={t(shown.links.length === 0 ? 'knowledge.held.links.global' : 'knowledge.held.links.hint')}>
      {#if shown.links.length > 0}
        <ul class="flex flex-col gap-2">
          {#each shown.links as link, index (index)}
            <li class="flex flex-col gap-2.5 rounded-lg border border-line bg-surface-2 p-3">
              <span class="flex items-center gap-2.5 text-sm font-medium text-fg">
                <GitBranch size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />
                <span class="min-w-0 flex-1 truncate">{repoLabel(link.repo)}</span>
                <Badge size="lg" tone="neutral">{pathsBadge(link.globs.length)}</Badge>
              </span>

              {#if link.globs.length > 0}
                <ul class="flex flex-wrap gap-1.5">
                  {#each link.globs as glob, position (position)}
                    <li class="inline-flex h-7 items-center rounded-full bg-surface-3 px-3 font-mono text-[13px] text-fg-2">{glob}</li>
                  {/each}
                </ul>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </SectionCard>

    <FactsCard flat heading={t('knowledge.panel.details')} hint={t('knowledge.held.details.hint')} rows={heldFacts(shown)} />

    <SectionCard flat heading={t('knowledge.held.decide')} hint={t('knowledge.review.hint')}>
      {#if item.status === 'changesRequested'}
        <!-- What was asked of the agent stays in view while the proposal waits for its revision. -->
        <blockquote class="mb-3 flex flex-col gap-1 border-l-2 border-line-strong pl-3">
          <p class="text-xs text-fg-3">{t('knowledge.drafts.held.askedOfAgent')}</p>
          <p class={COMMENT_TEXT}>{item.comment}</p>
        </blockquote>
      {/if}

      <HeldDecide item={detail ?? shown} approvable={detail !== null && status !== 'error'} {busy} {followUp} onapprove={() => onapprove(detail ?? shown)} onrequest={(comment) => onrequest(shown, comment)} onreject={(reason) => onreject(shown, reason)} />
    </SectionCard>
  {/if}
</SidePanel>
