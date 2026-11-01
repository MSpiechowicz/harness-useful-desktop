<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import IconButton from '$lib/components/IconButton.svelte';
  import { NOTICE_LOOKS } from '$lib/components/notice';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { knowledgeImport as flow } from '$lib/stores/knowledgeImport.svelte';
  import X from '@lucide/svelte/icons/x';

  const look = NOTICE_LOOKS.info;
</script>

<!-- Some passages of the last import have no vector yet: keyword search finds them already, search by meaning does after this. -->
{#if flow.embeddingPending > 0 && !flow.open}
  <div class="flex shrink-0 items-center gap-3 border-b border-line px-4 py-2 {look.box}" role="status">
    <span class="flex h-5 shrink-0 items-center"><look.icon class={look.color} size={ICON.sm} aria-hidden="true" /></span>
    <p class="min-w-0 flex-1 text-sm text-fg">{tn('knowledge.embed.notice', flow.embeddingPending)}</p>
    <Button variant="ghost" onclick={() => void flow.embedNow()}>{t('knowledge.embed.action')}</Button>
    <IconButton icon={X} iconSize={ICON.sm} label={t('knowledge.embed.dismiss')} class="shrink-0" onclick={() => flow.dismissEmbedding()} />
  </div>
{/if}
