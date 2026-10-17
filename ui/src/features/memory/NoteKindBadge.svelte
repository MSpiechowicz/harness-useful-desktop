<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { NoteKind } from '$lib/ipc/bindings/NoteKind';

  import { KIND_LABELS } from './note-format';

  interface Props {
    kind: NoteKind;
  }

  let { kind }: Props = $props();

  const TONES = { decision: 'accent', fix: 'success', problem: 'danger', outcome: 'info', optimisation: 'warning', location: 'neutral', note: 'neutral' } as const satisfies Record<NoteKind, 'accent' | 'success' | 'danger' | 'info' | 'warning' | 'neutral'>;
</script>

<Badge tone={TONES[kind]}>{t(KIND_LABELS[kind])}</Badge>
