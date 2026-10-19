<script lang="ts">
  import Folder from '@lucide/svelte/icons/folder';
  import Globe from '@lucide/svelte/icons/globe';

  import DocField from '$lib/components/docs/DocField.svelte';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import { t } from '$lib/i18n/i18n.svelte';

  type Scope = 'repo' | 'global';

  interface Props {
    value: Scope;
    /** An existing item is being edited: its scope is kept, so neither choice can be made. */
    fixed: boolean;
    /** A project is open, so there is a repository to keep the item in. */
    projectOpen: boolean;
    /** The id of the label. */
    id: string;
    error: string | null;
    /** What the chosen scope means. */
    help: string;
  }

  let { value = $bindable(), fixed, projectOpen, id, error, help }: Props = $props();

  const scopes = $derived<StripOption<Scope>[]>([
    { value: 'global', label: t('memory.editor.scope.global'), icon: Globe, disabled: fixed },
    { value: 'repo', label: t('memory.editor.scope.repo'), icon: Folder, disabled: fixed || !projectOpen },
  ]);
</script>

<DocField label={t('memory.editor.scope')} labelId={id} {error} {help}>
  <div class="flex"><SegmentedControl label={t('memory.editor.scope')} options={scopes} bind:value /></div>
</DocField>
