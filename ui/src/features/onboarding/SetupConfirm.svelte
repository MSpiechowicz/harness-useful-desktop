<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import { t, tn, type MessageKey } from '$lib/i18n/i18n.svelte';

  import type { Installer, Plan } from './setup-model';

  /** The heading over what each installer changes besides its own files. */
  const EFFECTS_HEADING: Record<Installer, MessageKey> = {
    dashboard: 'onboarding.setup.confirm.effects.dashboard',
    memory: 'onboarding.setup.confirm.effects.memory',
    opencodeSkills: 'onboarding.setup.confirm.effects.skills',
  };

  interface Props {
    open: boolean;
    plan: Plan;
    /** Runs what the plan lists; the dialog closes first. */
    onconfirm: () => void;
    /** Default to the recommended setup's wording. */
    title?: string;
    body?: string;
    confirmLabel?: string;
  }

  let { open = $bindable(false), plan, onconfirm, title = t('onboarding.setup.confirm.title'), body = t('onboarding.setup.confirm.body'), confirmLabel = t('onboarding.setup.confirm.run') }: Props = $props();

  function confirm(): void {
    open = false;
    onconfirm();
  }
</script>

<!-- Nothing runs before this: it names every command and every file the run touches, exactly as they will be used. -->
<Dialog bind:open size="lg" {title} description={body}>
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
              <span class="text-fg-3">{item.creates ? t('onboarding.setup.confirm.creates') : tn('onboarding.setup.confirm.adds', item.entries)}</span>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    {#each plan.effects as group (group.owner)}
      {@const heading = t(EFFECTS_HEADING[group.owner])}
      <section class="flex flex-col gap-2" aria-label={heading}>
        <h3 class="text-xs font-medium tracking-wider text-fg-3 uppercase">{heading}</h3>
        <ul class="flex list-disc flex-col gap-1 pl-5 text-sm text-fg-2 marker:text-fg-3">
          {#each group.effects as effect (effect)}
            <li>{effect}</li>
          {/each}
        </ul>
      </section>
    {/each}
  </div>

  {#snippet footer()}
    <Button variant="ghost" onclick={() => (open = false)}>{t('onboarding.setup.confirm.cancel')}</Button>
    <Button variant="primary" onclick={confirm}>{confirmLabel}</Button>
  {/snippet}
</Dialog>
