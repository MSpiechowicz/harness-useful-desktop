<script lang="ts">
  import { portal } from '$lib/a11y/portal';
  import Button from '$lib/components/Button.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  interface Props {
    onshow: () => void;
  }

  let { onshow }: Props = $props();
</script>

<!-- The question is hidden so the chat can be read; this bar is the only way back to it. -->
<div use:portal class="fixed inset-x-0 bottom-6 z-[80] flex justify-center px-4 pointer-events-none" data-question-peek>
  <div role="status" class="pointer-events-auto flex items-center gap-3 rounded-xl border border-line-strong bg-glass py-2 pr-2 pl-4 shadow-pop backdrop-blur-xl">
    <span class="text-base text-fg">{t('chat.request.showQuestion')}</span>
    <Button variant="primary" onclick={onshow}>{t('chat.request.showQuestionAction')}</Button>
  </div>
</div>
