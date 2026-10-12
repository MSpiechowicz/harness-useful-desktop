<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import Info from '@lucide/svelte/icons/info';

  import Button from '$lib/components/Button.svelte';
  import CopyButton from '$lib/components/CopyButton.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import Disclosure from '$lib/components/Disclosure.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { CliResult } from '$lib/ipc/bindings/CliResult';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';

  import { hasNoPreview, OUTCOME_TEXT, scopeSentence } from './action-model';
  import { actionRunner as runner } from './action-runner.svelte';

  const request = $derived(runner.request);
  const open = $derived(runner.phase !== 'idle' && request !== null);
  const name = $derived(request ? (harness.get(request.harness)?.info.name ?? request.harness) : '');
  const projectName = $derived(project.recents.find((entry) => entry.path === request?.project)?.name ?? project.active?.name ?? null);

  // The exact command line. Before the run it is the one confirming will start (never the preview's flag); afterwards, the one that ran.
  const ran = $derived(runner.result !== null);
  const command = $derived(((ran ? runner.result?.argv : runner.preview?.command) ?? []).join(' '));

  let logOpen = $state(false);
  $effect(() => {
    logOpen = runner.phase === 'done' && runner.failure !== null;
  });

  function output(result: CliResult): string {
    return [result.stdout, result.stderr].filter((part) => part.trim() !== '').join('\n');
  }
</script>

{#if open && request}
  <Dialog
    open
    size="md"
    title={request.title}
    description={t(request.scope === 'project' ? 'extensions.dialog.descriptionProject' : 'extensions.dialog.descriptionUser', { name })}
    dismissible={runner.phase !== 'running'}
    onclose={() => runner.close()}
  >
    <div class="flex flex-col gap-4">
      <p class="text-sm text-fg-2">{scopeSentence(request, projectName)}</p>

      {#if command}
        <section class="flex flex-col gap-1.5" aria-label={t('extensions.dialog.command')}>
          <h3 class="text-xs font-medium tracking-wider text-fg-3 uppercase">{ran ? t('extensions.dialog.commandRan') : t('extensions.dialog.commandWillRun')}</h3>
          <div class="flex items-start gap-1 rounded-md border border-line bg-surface-2 py-1 pr-1 pl-3">
            <code class="min-w-0 flex-1 py-1 font-mono text-sm leading-snug [overflow-wrap:anywhere] whitespace-pre-wrap text-fg">{command}</code>
            <CopyButton text={command} label={t('extensions.dialog.copyCommand')} />
          </div>
        </section>
      {/if}

      {#if runner.phase === 'previewing'}
        <p class="flex items-center gap-2 text-sm text-fg-2" role="status"><Spinner size={16} />{t('extensions.dialog.checking')}</p>
      {:else if runner.error && runner.phase === 'ready'}
        {@render problem(t('extensions.dialog.cannotRun'), runner.error)}
      {:else if runner.phase === 'ready' && runner.preview}
        {@const preview = runner.preview}

        <section class="flex flex-col gap-1.5" aria-label={t('extensions.dialog.preview')}>
          <h3 class="text-xs font-medium tracking-wider text-fg-3 uppercase">{t('extensions.dialog.preview')}</h3>

          {#if hasNoPreview(preview)}
            <p class="flex items-start gap-2 rounded-md border border-line bg-surface-2 px-3 py-2 text-sm text-fg-2">
              <Info size={16} class="mt-0.5 shrink-0 text-info" aria-hidden="true" />
              {t('extensions.dialog.noPreview')}
            </p>
          {:else if !runner.confirmable}
            {@render problem(t('extensions.dialog.previewProblem'), preview.summary ?? t('extensions.failure.default'))}
            {#if output(preview)}{@render log(output(preview), t('extensions.dialog.previewOutput'))}{/if}
          {:else}
            {@render log(output(preview) || t('extensions.dialog.previewEmpty'), t('extensions.dialog.previewOutput'))}
            <p class="text-xs text-fg-3">{t('extensions.dialog.nothingChanged')}</p>
            <p class="text-xs text-fg-3 [overflow-wrap:anywhere]">{t('extensions.dialog.previewedWith')} <code class="font-mono">{preview.argv.join(' ')}</code></p>
          {/if}
        </section>
      {:else if runner.phase === 'running'}
        <p class="flex items-center gap-2 text-sm text-fg-2" role="status"><Spinner size={16} />{t('extensions.dialog.running')}</p>
      {:else if runner.phase === 'done'}
        {#if runner.failure !== null || runner.error}
          {@render problem(runner.error ? t('extensions.dialog.couldNotRun') : t('extensions.dialog.failed'), runner.error ?? runner.failure ?? '')}
        {:else}
          <p class="flex items-start gap-2 rounded-md border border-line bg-success-wash px-3 py-2.5 text-sm text-fg" role="status">
            <CircleCheck size={16} class="mt-0.5 shrink-0 text-success" aria-hidden="true" />
            <span class="flex flex-col"><span class="font-medium">{runner.result?.summary ?? t('extensions.dialog.done')}</span><span class="text-fg-2">{runner.outcome ? OUTCOME_TEXT[runner.outcome] : OUTCOME_TEXT.unchecked}</span></span>
          </p>
        {/if}

        {#if runner.result && output(runner.result)}
          <Disclosure label={t('extensions.dialog.outputLog')} bind:open={logOpen}>
            {@render log(`${output(runner.result)}${runner.result.truncated ? `\n${t('extensions.dialog.truncated')}` : ''}`, t('extensions.dialog.commandOutput'))}
          </Disclosure>
        {/if}
      {/if}
    </div>

    {#snippet footer()}
      {#if runner.phase === 'done'}
        <Button variant="primary" onclick={() => runner.close()}>{t('extensions.dialog.close')}</Button>
      {:else}
        <Button variant="ghost" disabled={runner.phase === 'running'} onclick={() => runner.close()}>{t('extensions.dialog.cancel')}</Button>
        <Button
          variant={request.destructive ? 'danger' : 'primary'}
          loading={runner.phase === 'running'}
          disabled={!runner.confirmable}
          onclick={() => void runner.confirm()}
        >
          {request.verb}
        </Button>
      {/if}
    {/snippet}
  </Dialog>
{/if}

{#snippet problem(title: string, detail: string)}
  <p class="flex items-start gap-2 rounded-md border border-line bg-danger-wash px-3 py-2.5 text-sm text-fg" role="alert">
    <CircleAlert size={16} class="mt-0.5 shrink-0 text-danger" aria-hidden="true" />
    <span class="flex min-w-0 flex-col"><span class="font-medium">{title}</span><span class="text-fg-2 [overflow-wrap:anywhere]">{detail}</span></span>
  </p>
{/snippet}

{#snippet log(text: string, label: string)}
  <!-- Output is shown as plain text, whitespace kept; it is never parsed as markup. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -- a scrollable region must be reachable by keyboard -->
  <pre aria-label={label} class="max-h-48 overflow-auto rounded-md border border-line bg-surface-2 p-3 font-mono text-xs whitespace-pre-wrap text-fg-2" tabindex="0">{text}</pre>
{/snippet}
