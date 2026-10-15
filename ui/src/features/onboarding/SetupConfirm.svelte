<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import { setup } from './setup-state.svelte';

  const plan = $derived(setup.plan);

  function confirm(): void {
    setup.confirming = false;
    void setup.run();
  }
</script>

<!-- Nothing runs before this: it names every command and every file the run touches, exactly as they will be used. -->
<Dialog bind:open={setup.confirming} size="lg" title={t('onboarding.setup.confirm.title')} description={t('onboarding.setup.confirm.body')}>
  <div class="flex flex-col gap-5">
    {#if plan.commands.length > 0}
      <section class="flex flex-col gap-2" aria-label={t('onboarding.setup.confirm.commands')}>
        <h3 class="text-xs font-medium tracking-wider text-fg-3 uppercase">{t('onboarding.setup.confirm.commands')}</h3>
        <ul class="flex flex-col gap-2">
          {#each plan.commands as item (item.command)}
            <li class="rounded-md border border-line bg-surface-2 px-3 py-2">
              <code class="font-mono text-xs leading-snug [overflow-wrap:anywhere] whitespace-pre-wrap text-fg">{item.command}</code>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    {#if plan.files.length > 0}
      <section class="flex flex-col gap-2" aria-label={t('onboarding.setup.confirm.files')}>
        <h3 class="text-xs font-medium tracking-wider text-fg-3 uppercase">{t('onboarding.setup.confirm.files')}</h3>
        <ul class="flex flex-col gap-1.5">
          {#each plan.files as item (item.file)}
            <li class="flex items-baseline gap-2 text-sm text-fg-2">
              <code class="font-mono text-xs text-fg [overflow-wrap:anywhere]">{item.file}</code>
              <span class="text-fg-3">{item.creates ? t('onboarding.setup.confirm.creates') : t('onboarding.setup.confirm.adds')}</span>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    {#if plan.sideEffects.length > 0}
      <section class="flex flex-col gap-2" aria-label={t('onboarding.setup.confirm.effects')}>
        <h3 class="text-xs font-medium tracking-wider text-fg-3 uppercase">{t('onboarding.setup.confirm.effects')}</h3>
        <ul class="flex list-disc flex-col gap-1 pl-5 text-sm text-fg-2 marker:text-fg-3">
          {#each plan.sideEffects as effect (effect)}
            <li>{effect}</li>
          {/each}
        </ul>
      </section>
    {/if}
  </div>

  {#snippet footer()}
    <Button variant="ghost" onclick={() => (setup.confirming = false)}>{t('onboarding.setup.confirm.cancel')}</Button>
    <Button variant="primary" onclick={confirm}>{t('onboarding.setup.confirm.run')}</Button>
  {/snippet}
</Dialog>
