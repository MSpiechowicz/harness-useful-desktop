<script lang="ts">
  import GitBranch from '@lucide/svelte/icons/git-branch';
  import Plus from '@lucide/svelte/icons/plus';

  import FilterMenu from '$lib/components/FilterMenu.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import type { RepoChoice } from './knowledge-repos';

  interface Props {
    /** The repositories that can still be linked. */
    choices: RepoChoice[];
    /** A repository was chosen, by its key. */
    onpick: (key: string) => void;
    /** The user may not change the links: the button is off, and `describedby` names the text that says why. */
    disabled?: boolean;
    describedby?: string;
  }

  let { choices, onpick, disabled = false, describedby }: Props = $props();

  const options = $derived(choices.map((choice) => ({ value: choice.key, label: choice.label, hint: choice.hint, reason: choice.reason, disabled: choice.reason !== undefined, icon: GitBranch })));
</script>

<!-- The menu of the Scope filter, as a button that links: the same search above the choices and the same rows. It opens on the side with more room, as the editor sits in a scrolling panel or dialog. -->
<FilterMenu
  label={t('knowledge.links.add')}
  action={{ label: t('knowledge.links.add'), icon: Plus }}
  {options}
  value=""
  onchange={onpick}
  align="start"
  side="auto"
  {disabled}
  {describedby}
  search={{ label: t('components.scope.search'), placeholder: t('components.scope.search.placeholder'), empty: t('components.scope.search.empty') }}
/>
