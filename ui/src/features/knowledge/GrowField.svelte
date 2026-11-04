<script lang="ts">
  import { uid } from '$lib/utils/ids';

  interface Props {
    value: string;
    /** What the field is for, above it. */
    label: string;
    /** A quiet word at the end of the label line: "Required" or "Optional". */
    note?: string;
    placeholder?: string;
    disabled?: boolean;
    /** The longest text in characters. */
    maxlength?: number;
    /** Ctrl or Cmd with Enter sends. */
    onsubmit?: () => void;
    /** Escape leaves the field. */
    oncancel?: () => void;
  }

  let { value = $bindable(), label, note, placeholder, disabled = false, maxlength, onsubmit, oncancel }: Props = $props();

  const id = uid('grow-field');

  /** At least four rows tall, then as tall as its text up to a limit, so what was written is always in view. */
  function grow(node: HTMLTextAreaElement, _text: string) {
    const fit = (): void => {
      node.style.height = 'auto';
      node.style.height = `${node.scrollHeight}px`;
    };

    fit();

    return { update: fit };
  }

  function onkeydown(event: KeyboardEvent): void {
    if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      onsubmit?.();
    } else if (event.key === 'Escape' && oncancel) {
      event.preventDefault();
      event.stopPropagation();
      oncancel();
    }
  }
</script>

<div class="flex flex-col gap-1.5">
  <label for={id} class="flex items-baseline justify-between gap-3 text-sm font-medium text-fg-2">
    <span>{label}</span>
    {#if note}<span class="text-xs font-normal text-fg-3">{note}</span>{/if}
  </label>

  <textarea
    {id}
    bind:value
    use:grow={value}
    {onkeydown}
    {placeholder}
    {disabled}
    {maxlength}
    rows="4"
    class="max-h-80 w-full resize-none overflow-y-auto rounded-md border border-line bg-surface-2 px-3 py-2 text-base leading-relaxed text-fg outline-none placeholder:text-fg-3 focus:border-focus disabled:cursor-not-allowed disabled:opacity-50"
  ></textarea>
</div>
