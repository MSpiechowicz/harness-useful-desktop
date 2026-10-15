<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import { ICON, type IconComponent } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import { uid } from '$lib/utils/ids';

  interface Props {
    icon: IconComponent;
    title: string;
    /** What happens, in order: two or three short steps. */
    steps: string[];
    /** What it does, in a sentence: the line under the title. */
    does: string;
    never: string;
    action: string;
    /** The quiet line beside the button. */
    hint: string;
    onaction: () => void;
  }

  let { icon: Icon, title, steps, does, never, action, hint, onaction }: Props = $props();

  const headingId = uid('sync-card');
</script>

<!--
  Both cards share one structure and the grid gives them one height; side by side they take four rows of the section's
  grid (subgrid), so the header, the steps, "what it never does" and the button sit level with the other card's, whatever the
  number of steps. Stacked, each is a plain column. Both cards share one structure: a large tile with the title and what it does, the
  steps as a vertical timeline, what it never does, and the button at the foot. Whitespace, not dividers, separates them.
-->
<article aria-labelledby={headingId} class="flex flex-col gap-6 rounded-lg border-card border-line bg-surface-1 p-6 shadow-raised @xl:row-span-4 @xl:grid @xl:grid-rows-subgrid">
  <header class="flex items-start gap-4">
    <span class="grid size-12 shrink-0 place-items-center rounded-lg bg-accent-wash text-accent-text">
      <Icon size={ICON.lg} aria-hidden="true" />
    </span>

    <div class="flex min-w-0 flex-col gap-0.5">
      <h3 id={headingId} class="text-xl leading-tight text-fg">{title}</h3>
      <p class="text-sm text-fg-2">{does}</p>
    </div>
  </header>

  <!-- The flow as a timeline: a numbered node per step on one thin line, the step beside its node. -->
  <ol aria-label={t('sync.card.flow')} class="flex flex-col">
    {#each steps as step, index (step)}
      <li class="relative flex items-center gap-3 pb-7 last:pb-0">
        {#if index < steps.length - 1}
          <span class="absolute top-[calc(1.75rem+4px)] bottom-[4px] left-3.5 w-0.5 -translate-x-1/2 rounded-full bg-accent/45" aria-hidden="true"></span>
        {/if}
        <span class="relative grid size-7 shrink-0 place-items-center rounded-full bg-accent-fill text-xs font-semibold text-on-accent tabular" aria-hidden="true">{index + 1}</span>
        <span class="text-sm text-fg">{step}</span>
      </li>
    {/each}
  </ol>

  <div class="flex flex-col gap-1">
    <p class="text-xs font-medium tracking-wider text-fg-3 uppercase">{t('sync.card.never')}</p>
    <p class="text-sm text-fg-2">{never}</p>
  </div>

  <footer class="mt-auto flex items-center gap-3 pt-1">
    <Button variant="primary" onclick={onaction}>{action}</Button>
    <p class="ml-auto min-w-0 text-right text-xs text-fg-3">{hint}</p>
  </footer>
</article>
