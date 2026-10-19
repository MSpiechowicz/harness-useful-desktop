<script lang="ts">
  import DocSelect from '$lib/components/docs/DocSelect.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HookEvent } from '$lib/ipc/bindings/HookEvent';

  import { EVENT_HINTS, EVENT_ICONS, EVENT_LABELS, EVENTS } from './hook-format';

  interface Props {
    value: HookEvent;
    /** The id of the visible label that names the field. */
    labelledby: string;
    onchange: (event: HookEvent) => void;
  }

  let { value, labelledby, onchange }: Props = $props();

  const options = $derived(EVENTS.map((event) => ({ value: event, label: t(EVENT_LABELS[event]), hint: t(EVENT_HINTS[event]), icon: EVENT_ICONS[event] })));
</script>

<!-- The five moments a hook can fire at, each with a line on when it happens. -->
<DocSelect label={t('hooks.field.when')} {options} {value} {labelledby} {onchange} />
