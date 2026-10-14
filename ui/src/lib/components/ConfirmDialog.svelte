<script lang="ts" module>
  import type { IconComponent } from './icon';

  /** What a confirmation acts on: one row with an icon, a name and a line of details. */
  export interface ConfirmSubject {
    icon: IconComponent;
    name: string;
    details?: string;
    /** Something to double-check, in the warning tone: e.g. that the files sit in a folder a project chose. */
    note?: string;
  }
</script>

<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';

  import Button from './Button.svelte';
  import Dialog from './Dialog.svelte';
  import { ICON } from './icon';

  interface Props {
    open?: boolean;
    title: string;
    description: string;
    /** What the action applies to, e.g. a session: shown under the text so it is clear what goes. */
    subject?: ConfirmSubject;
    /** The destructive action's verb: "Stop chat". */
    confirmLabel: string;
    cancelLabel?: string;
    onconfirm: () => void;
  }

  let { open = $bindable(false), title, description, subject, confirmLabel, cancelLabel, onconfirm }: Props = $props();

  function confirm(): void {
    open = false;
    onconfirm();
  }
</script>

<!-- Destructive actions are confirmed in the danger tone; the safe choice comes first and holds the focus. -->
{#snippet row(item: ConfirmSubject)}
  <div class="flex items-start gap-3">
    <item.icon size={ICON.sm} class="mt-[3px] shrink-0 text-fg-3" aria-hidden="true" />
    <div class="min-w-0">
      <p class="line-clamp-2 font-medium break-words text-fg">{item.name}</p>
      {#if item.details}<p class="tabular truncate text-xs text-fg-3">{item.details}</p>{/if}
      {#if item.note}<p class="mt-1 text-xs break-all text-warning">{item.note}</p>{/if}
    </div>
  </div>
{/snippet}

{#snippet named()}
  {#if subject}{@render row(subject)}{/if}
{/snippet}

<Dialog bind:open {title} {description} subject={subject ? named : undefined}>
  {#snippet footer()}
    <Button variant="ghost" onclick={() => (open = false)}>{cancelLabel ?? t('components.confirm.keep')}</Button>
    <Button variant="danger" onclick={confirm}>{confirmLabel}</Button>
  {/snippet}
</Dialog>
