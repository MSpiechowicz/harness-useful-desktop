<script lang="ts">
  import { tick, untrack } from 'svelte';

  import { t } from '$lib/i18n/i18n.svelte';

  interface Props {
    title: string;
    /** Saves the new title; answers whether it was stored. The field stays while it is refused. */
    onsave: (title: string) => Promise<boolean>;
    onleave: () => void;
  }

  let { title, onsave, onleave }: Props = $props();

  // What is edited starts from the title as it is, and is not the title: the title moves under it while it is saved.
  let value = $state(untrack(() => title));
  let field: HTMLInputElement | undefined = $state();
  let saving = $state(false);

  $effect(() => {
    void tick().then(() => {
      // The caret goes to the end: a selected title is a blue block that reads as a highlight, not as a field to type in.
      field?.focus();
      field?.setSelectionRange(field.value.length, field.value.length);
    });
  });

  async function commit(): Promise<void> {
    const next = value.trim();

    if (saving) {
      return;
    }

    if (next === '' || next === title) {
      onleave();

      return;
    }

    saving = true;

    if (await onsave(next)) {
      onleave();
    }

    saving = false;
    field?.focus();
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === 'Enter') {
      event.preventDefault();
      void commit();
    } else if (event.key === 'Escape') {
      // Escape closes the panel too: this one only puts the field away.
      event.preventDefault();
      event.stopPropagation();
      onleave();
    }
  }
</script>

<!-- The header shows the field instead of the title and the line under it (the band is a fixed height and centres it), in the title's own type. It is pulled left by its border and padding so the text starts where the title did. -->
<input
  bind:this={field}
  bind:value
  type="text"
  maxlength="300"
  aria-label={t('knowledge.panel.rename.label')}
  disabled={saving}
  autocomplete="off"
  spellcheck="false"
  class="-ml-[calc(var(--spacing)*3+1px)] h-control min-w-0 flex-1 rounded-md border border-line-strong bg-surface-2 px-3 text-lg font-semibold text-fg outline-none focus-visible:border-accent/50"
  onkeydown={onKeydown}
  onblur={() => void commit()}
/>
