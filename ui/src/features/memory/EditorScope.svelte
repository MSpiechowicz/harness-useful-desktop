<script lang="ts">
  import ScopeSelect from '$lib/components/scope/ScopeSelect.svelte';
  import type { ScopeTarget } from '$lib/components/scope/scope-options';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';

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
  }

  let { value = $bindable(), projects, id, error, help, fixed }: Props = $props();
</script>

<ScopeSelect label={t('components.scope.appliesTo')} labelId={id} {projects} {value} onchange={(next) => (value = next)} {help} {error} disabled={fixed} />
