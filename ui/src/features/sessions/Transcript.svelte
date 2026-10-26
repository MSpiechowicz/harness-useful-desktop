<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import GitFork from '@lucide/svelte/icons/git-fork';
  import { untrack } from 'svelte';

  import { provideToolContext } from '$lib/chat/tool-context';
  import Button from '$lib/components/Button.svelte';
  import NoticeBanner from '$lib/components/NoticeBanner.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import VirtualList from '$lib/components/VirtualList.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { SessionDetail } from '$lib/stores/sessions.svelte';
  import { cx } from '$lib/utils/cx';
  import { formatCount } from '$lib/utils/format';

  import MessageRow from '../chat/MessageRow.svelte';
  import { rowMessage } from './transcript-focus';

  interface Props {
    detail: SessionDetail;
    project: string;
    /** Starts a new session from one of the user's messages; without it the transcript is read-only. */
    onfork?: (id: string) => void;
    /** Disables the fork buttons while a fork or resume is under way. */
    busy?: boolean;
  }

  let { detail, project, onfork, busy = false }: Props = $props();

  const ESTIMATED_ROW_PX = 96;

  /** The id of the user message in row `index`, or `null` for any other row. */
  function userMessageId(index: number): string | null {
    const message = rowMessage(detail.messages, index);

    return message?.role === 'user' ? (message.id ?? '') : null;
  }

  provideToolContext({
    project: untrack(() => project),
    get label() {
      return t('sessions.touched.source');
    },
  });

  const chat = $derived(detail.transcript);
</script>

<div class="flex size-full min-h-0 flex-col">
  {#if detail.transcriptStatus === 'loading'}
    <div class="flex flex-col gap-3 p-6" role="status" aria-label={t('sessions.transcript.loading')}>
      <Skeleton width="60%" height="2.5rem" />
      <Skeleton height="4rem" />
      <Skeleton width="80%" height="2.5rem" />
    </div>
  {:else if detail.transcriptStatus === 'error'}
    <div class="flex flex-col items-center gap-3 p-8 text-center" role="alert">
      <CircleAlert size={24} class="text-danger" aria-hidden="true" />
      <p class="text-base text-fg-2">{detail.transcriptError?.message ?? t('sessions.transcript.unreadable')}</p>
      <Button onclick={() => detail.reload()}>{t('sessions.tryAgain')}</Button>
    </div>
  {:else if chat.items.length === 0}
    <p class="grid size-full place-items-center p-8 text-center text-base text-fg-3" role="status">{t('sessions.transcript.empty')}</p>
  {:else}
    {#if detail.truncated}
      <div class="notice-band border-b border-line py-3"><NoticeBanner notices={[{ level: 'info', message: t('sessions.transcript.truncated') }]} /></div>
    {/if}
    <VirtualList items={chat.items} variable estimate={ESTIMATED_ROW_PX} overscan={10} label={t('sessions.transcript.label')} key={(item) => item.key} class="min-h-0 flex-1">
      {#snippet row(item, index)}
        {@const userId = onfork ? userMessageId(index) : null}
        <div class={cx('chat-col py-2', index === 0 && 'pt-6', index === chat.items.length - 1 && 'pb-8')}>
          {#if userId !== null}
            <!-- Every user message has the same labelled button under it, always visible: the new chat keeps the conversation
                 up to and including this message; everything after it is left out (the original session stays as it was). -->
            <div class="flex flex-col items-end gap-0.5">
              <div class="max-w-[85%] min-w-0 [&_article]:max-w-full">
                <MessageRow {item} {chat} />
              </div>
              <button
                type="button"
                title={t('sessions.transcript.forkHint')}
                disabled={busy || userId === ''}
                onclick={() => onfork?.(userId)}
                class="inline-flex h-control items-center gap-1.5 rounded-md px-2 text-sm text-fg-3 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:bg-transparent disabled:hover:text-fg-3"
              >
                <GitFork size={16} aria-hidden="true" />
                {t('sessions.transcript.fork')}
              </button>
            </div>
          {:else}
            <MessageRow {item} {chat} />
          {/if}

          {#if index === chat.items.length - 1 && detail.nextCursor !== null}
            <div class="mt-3 flex items-center justify-center gap-3">
              <span class="tabular text-xs text-fg-3">{tn('sessions.transcript.progress', detail.total, { shown: formatCount(detail.messages.length) })}</span>
              <Button loading={detail.loadingMore} onclick={() => void detail.loadMore()}>{t('sessions.transcript.loadMore')}</Button>
            </div>
          {/if}
        </div>
      {/snippet}
    </VirtualList>
  {/if}
</div>
