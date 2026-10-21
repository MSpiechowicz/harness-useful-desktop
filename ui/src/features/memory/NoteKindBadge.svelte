<script lang="ts">
  import type { ComponentProps } from 'svelte';

  import Badge from '$lib/components/Badge.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { NoteKind } from '$lib/ipc/bindings/NoteKind';

  import { KIND_LABELS } from './note-format';

  interface Props {
    kind: NoteKind;
    /** `lg` in a table row, beside the other row badges. */
    size?: ComponentProps<typeof Badge>['size'];
  }

  let { kind, size }: Props = $props();

  const TONES = { decision: 'accent', fix: 'success', problem: 'danger', outcome: 'info', optimisation: 'warning', location: 'neutral', note: 'neutral' } as const satisfies Record<NoteKind, 'accent' | 'success' | 'danger' | 'info' | 'warning' | 'neutral'>;
</script>

<Badge tone={TONES[kind]} {size}>{t(KIND_LABELS[kind])}</Badge>
