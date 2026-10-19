<script lang="ts">
  import UsersRound from '@lucide/svelte/icons/users-round';

  import Badge from '../Badge.svelte';
  import type { IconComponent } from '../icon';
  import DocGroupHeader from './DocGroupHeader.svelte';
  import { type DocGroup, MAX_PER_SCOPE } from './doc-groups';

  interface Props {
    group: DocGroup<unknown>;
    /** How many columns the heading spans. */
    span: number;
    /** The name of the open project, which the project group is titled with. */
    projectName: string | null;
    /** Team documents run without local approval: said as a badge in the Team heading. */
    teamTrusted: boolean;
    labels: {
      global: string;
      project: string;
      invalid: string;
      team: string;
      trusted: string;
      addGlobal: string;
      addProject: string;
      /** "2 of 64". */
      count: (total: number) => string;
    };
    icons: { global: IconComponent; project: IconComponent };
    onadd: (scope: 'global' | 'project') => void;
  }

  let { group, span, projectName, teamTrusted, labels, icons, onadd }: Props = $props();
</script>

<!-- The heading of a group of the list: a scope with the way to add a document to it, the files to fix, or the team's. -->
{#if group.id === 'global' || group.id === 'project'}
  {@const scope = group.id}

  <DocGroupHeader
    {span}
    icon={icons[scope]}
    label={labels[scope]}
    name={scope === 'project' ? projectName : null}
    count={labels.count(group.total)}
    fixedLabel
    add={{ label: scope === 'global' ? labels.addGlobal : labels.addProject, disabled: group.total >= MAX_PER_SCOPE || (scope === 'project' && !projectName), onadd: () => onadd(scope) }}
  />
{:else}
  <DocGroupHeader {span} icon={group.id === 'team' ? UsersRound : undefined} label={group.id === 'team' ? labels.team : labels.invalid} count={String(group.total)}>
    {#snippet badge()}
      {#if group.id === 'team' && teamTrusted}<Badge tone="info" class="ml-1 shrink-0 normal-case">{labels.trusted}</Badge>{/if}
    {/snippet}
  </DocGroupHeader>
{/if}
