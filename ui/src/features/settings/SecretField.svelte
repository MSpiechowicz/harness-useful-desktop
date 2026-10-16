<script lang="ts">
  import Eye from '@lucide/svelte/icons/eye';
  import EyeOff from '@lucide/svelte/icons/eye-off';

  import IconButton from '$lib/components/IconButton.svelte';
  import { cx } from '$lib/utils/cx';

  interface Props {
    value: string;
    label: string;
    placeholder: string;
    /** Names of the eye button while the text is hidden and while it is shown. */
    showLabel: string;
    hideLabel: string;
    invalid?: boolean;
    /** Id of the element that explains the field, for assistive technology. */
    describedby?: string;
    onblur?: () => void;
  }

  let { value = $bindable(''), label, placeholder, showLabel, hideLabel, invalid = false, describedby, onblur }: Props = $props();

  let shown = $state(false);

  // Once the field is emptied (a save clears it), the next thing typed starts masked again.
  $effect(() => {
    if (value === '') {
      shown = false;
    }
  });
</script>

<!-- Masked like a password, but never offered to a password manager: it is a connection secret, not a login. -->
<div
  class={cx(
    'flex h-control items-center gap-1 rounded-md border bg-surface-2 pr-1 pl-3 transition-colors duration-[var(--dur-fast)] focus-within:border-focus',
    invalid ? 'border-danger' : 'border-line-strong hover:border-line-strong',
  )}
>
  <input
    bind:value
    {onblur}
    type={shown ? 'text' : 'password'}
    aria-label={label}
    aria-invalid={invalid ? 'true' : undefined}
    aria-describedby={describedby}
    {placeholder}
    autocomplete="off"
    autocapitalize="off"
    spellcheck="false"
    data-1p-ignore
    data-lpignore="true"
    class="h-full min-w-0 flex-1 bg-transparent font-mono text-base text-fg outline-none placeholder:font-sans placeholder:text-fg-3"
  />
  <!-- A small square inside the field: its hover fill stays clear of the field's border. -->
  <IconButton class="!size-7 shrink-0" icon={shown ? EyeOff : Eye} label={shown ? hideLabel : showLabel} iconSize={16} aria-pressed={shown} onclick={() => (shown = !shown)} />
</div>
