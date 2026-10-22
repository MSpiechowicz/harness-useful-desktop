<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
  import type { McpRefusal } from '$lib/ipc/bindings/McpRefusal';
  import { harness as harnesses } from '$lib/stores/harness.svelte';

  import SetupCommand from './SetupCommand.svelte';
  import { mcpSnippet, refusalText, type McpKind } from './setup-model';

  interface Props {
    kind: McpKind;
    /** Whose file it is, which decides the shape of the entry to add by hand. */
    harness: HarnessId;
    /** The column's title. */
    title: string;
    /** Why the app leaves the file alone, when it does. */
    refused: McpRefusal | null;
    /** The file has comments the app would lose by rewriting it: where to add the entry, and the entry to add. */
    manualEdit: { file: string; snippet: string } | null;
    /** The entry is in the file but switched off. */
    disabled: boolean;
    /** The entry cannot be added before its companion is installed: what to do about it. */
    waitsFor: string | null;
    failure: string | null;
  }

  let { kind, harness, title, refused, manualEdit, disabled, waitsFor, failure }: Props = $props();
</script>

<!-- What one MCP cell of a row cannot say; the dashboard's and the memory server's are built the same way. -->
<section class="flex flex-col gap-2" aria-label={title}>
  <h4 class="text-xs font-medium tracking-wider text-fg-3 uppercase">{title}</h4>

  {#if refused === 'cliFailed'}
    <!-- Claude Code's file is changed by its CLI only: a snippet to paste into it would be wrong advice. -->
    <p class="text-sm text-fg-2">{refusalText(refused)}</p>
  {:else if refused}
    <p class="text-sm text-fg-2">{t('onboarding.setup.mcp.refused', { reason: refusalText(refused) })}</p>
    <SetupCommand text={mcpSnippet(kind, harness, harnesses.openCodeGeneration)} copyLabel={t('onboarding.setup.copySnippet')} />
  {:else if manualEdit}
    <p class="text-sm text-fg-2">{t('onboarding.setup.mcp.manualEdit', { file: manualEdit.file })}</p>
    <SetupCommand text={manualEdit.snippet} copyLabel={t('onboarding.setup.copySnippet')} />
  {:else if disabled}
    <p class="text-sm text-fg-2">{t('onboarding.setup.mcp.disabled')}</p>
  {:else if waitsFor}
    <p class="text-sm text-fg-2">{waitsFor}</p>
  {/if}

  {#if failure}
    <p class="text-sm text-danger" role="alert">{t('onboarding.setup.mcp.failed', { reason: failure })}</p>
  {/if}
</section>
