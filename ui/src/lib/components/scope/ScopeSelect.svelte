<script lang="ts">
  import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';
  import { t } from '$lib/i18n/i18n.svelte';

  import DocField from '../docs/DocField.svelte';
  import DocSelect from '../docs/DocSelect.svelte';
  import { buildTargetOptions, type ScopeTarget, targetOf, targetValue } from './scope-options';

  interface Props {
    /** The name of the field, e.g. "Applies to". */
    label: string;
    /** The id of the label, which names the dropdown. */
    labelId: string;
    /** The projects on this computer: the only repositories a document can be saved to. */
    projects: ProjectScope[];
    value: ScopeTarget;
    onchange: (target: ScopeTarget) => void;
    /** A line on what the field is for. */
    help?: string;
    /** What is wrong, said in red. It takes the place of `help`. */
    error?: string | null;
    /** The scope is fixed (editing a document that cannot move): the field shows it and does not open. */
    disabled?: boolean;
  }

  let { label, labelId, projects, value, onchange, help, error = null, disabled = false }: Props = $props();

  const options = $derived(buildTargetOptions(projects, t('components.scope.global'), value));
</script>

<!-- Where a document applies: Global or one of the repositories on this computer, with a search for when there are many. -->
<DocField {label} {labelId} {help} {error}>
  <DocSelect
    {label}
    labelledby={labelId}
    {options}
    value={targetValue(value)}
    onchange={(next) => onchange(targetOf(next))}
    {disabled}
    search={{ label: t('components.scope.search'), placeholder: t('components.scope.search.placeholder'), empty: t('components.scope.search.empty') }}
  />
</DocField>
