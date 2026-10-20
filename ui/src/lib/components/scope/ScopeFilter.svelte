<script lang="ts">
  import Folder from '@lucide/svelte/icons/folder';
  import Globe from '@lucide/svelte/icons/globe';

  import { t } from '$lib/i18n/i18n.svelte';

  import FilterMenu from '../FilterMenu.svelte';
  import type { ScopeChoice, ScopeOption } from './scope-options';

  interface Props {
    /** The entries from `buildScopeOptions`: All first, then the repositories, Global last. */
    options: ScopeOption[];
    value: ScopeChoice;
    onchange: (choice: ScopeChoice) => void;
    /** A project without a repository key cannot be filtered by (the data source knows no key for it), so it is shown but cannot be chosen. */
    disableKeyless?: boolean;
    align?: 'start' | 'end';
  }

  let { options, value, onchange, disableKeyless = false, align = 'start' }: Props = $props();

  const entries = $derived(options.map(({ value: choice, label, hint, count, keyless }) => ({ value: choice, label, hint, count, disabled: disableKeyless && keyless })));
</script>

<!-- "Scope: All" in the row of filters: the repositories are many, so the menu is searchable and its list keeps one height. -->
<FilterMenu
  label={t('components.scope.label')}
  prefix={t('components.scope.label')}
  icon={value === 'global' ? Globe : Folder}
  options={entries}
  {value}
  {onchange}
  onclear={() => onchange('all')}
  clearLabel={t('components.scope.clear')}
  {align}
  search={{ label: t('components.scope.search'), placeholder: t('components.scope.search.placeholder'), empty: t('components.scope.search.empty') }}
/>
