<script lang="ts">
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import Link from '@lucide/svelte/icons/link';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';

  import Spinner from '$lib/components/Spinner.svelte';
  import CopyButton from '$lib/components/CopyButton.svelte';
  import { NOTICE_LOOKS } from '$lib/components/notice';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { BannerItem, BashItem, NoticeItem, OutputItem, UrlItem } from '$lib/chat/types';

  import TerminalBlock from './TerminalBlock.svelte';

  interface Props {
    item: NoticeItem | OutputItem | BannerItem | UrlItem | BashItem;
  }

  let { item }: Props = $props();

  const exit = $derived(item.kind === 'bash' ? (item.cancelled ? t('chat.note.cancelled') : item.exitCode === null ? '' : t('chat.note.exit', { code: item.exitCode })) : '');
</script>

{#if item.kind === 'notice'}
  {@const look = NOTICE_LOOKS[item.level]}
  <p class="chat-radius flex items-start gap-2.5 border border-line px-3.5 py-2 text-sm text-fg {look.box}" role={item.level === 'error' ? 'alert' : 'status'}>
    <look.icon size={16} class="mt-0.5 shrink-0 {look.color}" aria-hidden="true" />
    <span class="break-words">{item.message}</span>
  </p>
{:else if item.kind === 'output'}
  <TerminalBlock text={item.text} />
{:else if item.kind === 'compaction' || item.kind === 'retry'}
  <p class="flex items-center justify-center gap-2 py-1 text-xs text-fg-3" role="status">
    {#if item.phase === 'running'}
      <Spinner size={16} />
    {:else if item.phase === 'failed'}
      <TriangleAlert size={16} class="text-warning" aria-hidden="true" />
    {:else}
      <CircleCheck size={16} class="text-success" aria-hidden="true" />
    {/if}
    <span class="break-words">{item.text}</span>
  </p>
{:else if item.kind === 'url'}
  <div class="chat-radius flex flex-col gap-2 border border-line-strong bg-surface-1/60 px-3.5 py-3">
    <p class="flex items-center gap-2 text-sm font-medium text-fg"><Link size={16} class="text-accent-text" aria-hidden="true" /> {t('chat.note.link.title')}</p>
    {#if item.instructions}<p class="text-sm break-words text-fg-2">{item.instructions}</p>{/if}
    <div class="flex items-center gap-1 rounded-sm bg-canvas px-2.5 py-1.5">
      <code class="min-w-0 flex-1 font-mono text-xs break-all text-fg">{item.url}</code>
      <CopyButton text={item.url} label={t('chat.note.link.copy')} control />
    </div>
    <p class="text-xs text-fg-3">{t('chat.note.link.warning')}</p>
  </div>
{:else if item.kind === 'bash'}
  <div class="flex flex-col gap-1">
    <TerminalBlock text={item.output} command={item.command} running={item.running} />
    {#if !item.running}
      <p class="tabular text-xs text-fg-3">{[exit, item.truncated ? t('chat.note.truncated') : ''].filter(Boolean).join(' · ')}</p>
    {/if}
  </div>
{/if}
