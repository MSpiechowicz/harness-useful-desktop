<script lang="ts">
  import { autofocus } from '$lib/a11y/autofocus';
  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import type { InstallScope } from '$lib/ipc/bindings/InstallScope';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { uid } from '$lib/utils/ids';

  import { actionRequest, targetProblem, type ActionRequest, type ComposeKind } from './action-model';

  interface Props {
    kind: ComposeKind;
    onsubmit: (request: ActionRequest) => void;
    onclose: () => void;
  }

  let { kind, onsubmit, onclose }: Props = $props();

  const COPY = {
    plugin: {
      title: 'Install a plugin',
      description: 'You will see the exact command and a preview before anything is installed.',
      label: 'Plugin',
      placeholder: 'plugin-name@marketplace',
      hint: 'A plugin of a marketplace you added, written name@marketplace, or an npm package name.',
      scoped: true,
    },
    marketplace: {
      title: 'Add a marketplace',
      description: 'A marketplace is a catalog of plugins. Adding one installs nothing by itself.',
      label: 'Marketplace source',
      placeholder: 'owner/repository',
      hint: 'A GitHub repository (owner/repo) or a git address.',
      scoped: false,
    },
    package: {
      title: 'Install a package',
      description: 'You will see the exact command before anything is installed.',
      label: 'Package source',
      placeholder: 'npm:package-name',
      hint: 'npm:name, git:host/owner/repo, or a local path.',
      scoped: true,
    },
  } as const;

  const copy = $derived(COPY[kind]);
  const hintId = uid('install-hint');
  const projectPath = $derived(project.active?.path ?? null);
  const scopes = $derived<StripOption<InstallScope>[]>([
    { value: 'user', label: 'For me' },
    { value: 'project', label: project.active ? `This project (${project.active.name})` : 'This project', disabled: projectPath === null },
  ]);

  let value = $state('');
  let scope = $state<InstallScope>('user');
  // An empty field is not an error until the user tries to continue; leaving it half-typed is.
  let touched = $state(false);

  const problem = $derived(targetProblem(value));
  const shownProblem = $derived(touched && problem ? problem : null);

  function submit(): void {
    touched = true;

    const active = harness.active;
    if (problem || !active) {
      return;
    }

    const text = value.trim();
    const action =
      kind === 'plugin'
        ? ({ type: 'pluginInstall', target: text, scope } as const)
        : kind === 'marketplace'
          ? ({ type: 'marketplaceAdd', source: text } as const)
          : ({ type: 'packageInstall', source: text } as const);

    onsubmit(actionRequest(active.info.id, action, copy.scoped ? scope : 'user', projectPath));
  }
</script>

<Dialog open title={copy.title} description={copy.description} {onclose}>
  <form
    class="flex flex-col gap-4"
    onsubmit={(event) => {
      event.preventDefault();
      submit();
    }}
  >
    <label class="flex flex-col gap-1.5 text-sm text-fg-2">
      {copy.label}
      <input
        bind:value
        use:autofocus
        placeholder={copy.placeholder}
        autocomplete="off"
        spellcheck="false"
        aria-describedby={hintId}
        aria-invalid={shownProblem ? 'true' : undefined}
        onblur={() => (touched ||= value.trim() !== '')}
        class="h-10 w-full rounded-md border bg-surface-2 px-3 font-mono text-base text-fg outline-none focus:border-focus {shownProblem ? 'border-danger' : 'border-line-strong'}"
      />
      <span id={hintId} class="text-xs {shownProblem ? 'text-danger' : 'text-fg-3'}" role={shownProblem ? 'alert' : undefined}>{shownProblem ?? copy.hint}</span>
    </label>

    {#if copy.scoped}
      <div class="flex flex-col gap-1.5 text-sm text-fg-2">
        <span>Install for</span>
        <SegmentedControl label="Install for" options={scopes} bind:value={() => scope, (next) => (scope = next)} />
        {#if projectPath === null}<span class="text-xs text-fg-3">Open a project to install into it.</span>{/if}
      </div>
    {/if}

    <!-- Enter in the field submits through this hidden button. -->
    <button type="submit" class="hidden" tabindex="-1" aria-hidden="true">Review</button>
  </form>

  {#snippet footer()}
    <Button variant="ghost" onclick={onclose}>Cancel</Button>
    <Button variant="primary" disabled={problem !== null && touched} onclick={submit}>Review</Button>
  {/snippet}
</Dialog>
