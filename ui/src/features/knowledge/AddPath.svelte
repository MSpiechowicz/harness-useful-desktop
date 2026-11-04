<script lang="ts">
  import Plus from '@lucide/svelte/icons/plus';
  import { tick } from 'svelte';
  import { fade } from 'svelte/transition';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import { scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';

  interface Props {
    /** Names the field: the repository it adds a path to. */
    repo: string;
    /** A glob was entered. */
    onadd: (glob: string) => void;
    /** The button that opens the field is off (a read-only role). */
    disabled?: boolean;
    /** The id of the text that says why it is off. */
    describedby?: string;
  }

  let { repo, onadd, disabled = false, describedby }: Props = $props();

  let editing = $state(false);
  let value = $state('');
  let field: HTMLInputElement | undefined = $state();
  let button: HTMLElement | undefined = $state();

  async function reveal(): Promise<void> {
    editing = true;
    await tick();
    field?.focus();
  }

  async function leave(): Promise<void> {
    editing = false;
    value = '';
    await tick();
    button?.querySelector('button')?.focus();
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === 'Enter') {
      event.preventDefault();

      const glob = value.trim();

      if (glob !== '') {
        onadd(glob);
      }

      void leave();
    } else if (event.key === 'Escape') {
      // The panel closes on Escape too: this one only cancels the field.
      event.preventDefault();
      event.stopPropagation();
      void leave();
    }
  }
</script>

<!-- A secondary button, like every action of a panel, that reveals an ordinary field, not a box that is always there: Enter adds the glob, Escape puts it away. -->
{#if editing}
  <input
    bind:this={field}
    bind:value
    in:fade={{ duration: scaled(120) }}
    type="text"
    aria-label={t('knowledge.links.pathLabel', { repo })}
    placeholder={t('knowledge.links.pathPlaceholder')}
    autocomplete="off"
    spellcheck="false"
    class="h-control min-w-0 flex-1 rounded-md border border-line-strong bg-surface-2 px-3 font-mono text-sm text-fg outline-none placeholder:text-fg-3 focus-visible:border-accent/50"
    onkeydown={onKeydown}
    onblur={() => value.trim() === '' && void leave()}
  />
{:else}
  <span bind:this={button} class="contents">
    <Button aria-label={t('knowledge.links.addPathFor', { repo })} {disabled} aria-describedby={describedby} onclick={() => void reveal()}>
      {#snippet leading()}<Plus size={ICON.sm} aria-hidden="true" />{/snippet}
      {t('knowledge.links.addPath')}
    </Button>
  </span>
{/if}
