<script lang="ts">
  import ScopeSelect from '$lib/components/scope/ScopeSelect.svelte';
  import type { ScopeTarget } from '$lib/components/scope/scope-options';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';
  import { uid } from '$lib/utils/ids';

  import EditorSavePlace from './EditorSavePlace.svelte';
  import type { SavePlace } from './save-place.svelte';

  interface Props {
    value: ScopeTarget;
    /** The projects on this computer: where a new note or script can be kept. */
    projects: ProjectScope[];
    /** The id of the label. */
    id: string;
    error: string | null;
    /** What the chosen scope means. */
    help: string;
    /** An existing item is being edited: its scope is kept, so the field only shows it. */
    fixed: boolean;
    /** Where a new entry is saved: its field comes right after Applies to, when the scope is kept in more than one place. */
    place: SavePlace;
  }

  let { value = $bindable(), projects, id, error, help, fixed, place }: Props = $props();

  const placeId = uid('editor-place');
</script>

<ScopeSelect label={t('components.scope.appliesTo')} labelId={id} {projects} {value} onchange={(next) => (value = next)} {help} {error} disabled={fixed} />

{#if place.offered}
  <EditorSavePlace bind:value={place.value} id={placeId} offline={place.offline} />
{/if}
