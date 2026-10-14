<script lang="ts">
  import RotateCw from '@lucide/svelte/icons/rotate-cw';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';

  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  import Button from './Button.svelte';
  import CopyButton from './CopyButton.svelte';
  import Disclosure from './Disclosure.svelte';
  import EmptyState from './EmptyState.svelte';

  interface Props {
    /** What failed, as the start of a sentence: "The Ecosystem page", "Harness Desktop". */
    area: string;
    error: unknown;
    /** Renders the failed part again; without it only a full reload is offered. */
    retry?: () => void;
    /** `screen` covers the window (nothing else may be usable); `view` fills its slot. */
    variant?: 'view' | 'screen';
  }

  let { area, error, retry, variant = 'view' }: Props = $props();

  const message = $derived(error instanceof Error ? `${error.name}: ${error.message}` : String(error));
  const details = $derived(error instanceof Error && error.stack ? error.stack : message);
  const body = $derived(t(variant === 'screen' ? 'components.error.screen' : 'components.error.view', { area }));
</script>

<div
  role="alert"
  class={cx('grid place-items-center overflow-auto', variant === 'screen' ? 'fixed inset-0 z-[100] bg-canvas' : 'size-full min-h-0')}
>
  <EmptyState icon={TriangleAlert} title={t('components.error.title')} {body}>
    {#snippet action()}
      <div class="flex w-full max-w-md flex-col items-center gap-5">
        <div class="flex gap-2">
          {#if retry}
            <Button variant="primary" onclick={retry}>
              {#snippet leading()}<RotateCw size={16} aria-hidden="true" />{/snippet}
              {t('components.error.retry')}
            </Button>
          {/if}
          <Button variant={retry ? 'secondary' : 'primary'} onclick={() => location.reload()}>{t('components.error.reload')}</Button>
        </div>

        <Disclosure label={t('components.error.details')} class="w-full text-left">
          <div class="flex items-start gap-2 rounded-md border border-line bg-surface-2 p-3">
            <code class="min-w-0 flex-1 font-mono text-xs break-words whitespace-pre-wrap text-fg-2">{message}</code>
            <CopyButton text={details} label={t('components.error.copy')} control />
          </div>
        </Disclosure>
      </div>
    {/snippet}
  </EmptyState>
</div>
