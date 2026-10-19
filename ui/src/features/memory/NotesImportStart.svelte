<script lang="ts">
  import FileDown from '@lucide/svelte/icons/file-down';
  import Info from '@lucide/svelte/icons/info';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import CodeBlock from '$lib/components/CodeBlock.svelte';
  import { ICON } from '$lib/components/icon';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { NotesImportFormat } from '$lib/ipc/bindings/NotesImportFormat';

  import { LIMITS } from './note-form';
  import { IMPORT_EXAMPLES, IMPORT_KINDS } from './notes-import-example';
  import { notesImport as flow } from './notes-import-state.svelte';

  let format = $state<NotesImportFormat>('csv');

  const formats = $derived([
    { value: 'csv' as const, label: t('memory.import.format.csv') },
    { value: 'json' as const, label: t('memory.import.format.json') },
  ]);
  /** Each column, whether a note needs it, what it holds, and the values it takes when they are few. */
  const fields = $derived([
    { name: 'title', required: true, text: t('memory.import.start.title', { max: LIMITS.title }), values: [] },
    { name: 'body', required: true, text: t('memory.import.start.body', { max: LIMITS.body }), values: [] },
    { name: 'kind', required: false, text: t('memory.import.start.kind'), values: IMPORT_KINDS },
    { name: 'tags', required: false, text: t('memory.import.start.tags', { max: LIMITS.tags }), values: [] },
    { name: 'scope', required: false, text: t('memory.import.start.scope'), values: ['repo', 'global'] },
  ]);

  const HEADING = 'text-xs font-medium tracking-wide text-fg-3 uppercase';
</script>

<div class="flex flex-col gap-6">
  <section class="flex flex-col gap-3" aria-labelledby="notes-import-columns">
    <h3 id="notes-import-columns" class={HEADING}>{t('memory.import.start.columns')}</h3>

    <!-- One grid for every row, so the badges and the texts line up whatever the length of a column name. -->
    <dl class="grid grid-cols-[auto_auto_1fr] divide-y divide-line rounded-md border border-line">
      {#each fields as field (field.name)}
        <div class="col-span-3 grid grid-cols-subgrid items-start gap-x-4 px-4 py-3">
          <dt class="flex h-7 items-center font-mono text-sm text-fg">{field.name}</dt>
          <dd class="flex h-7 items-center">
            <Badge tone={field.required ? 'accent' : 'neutral'} size="lg">{field.required ? t('memory.import.start.required') : t('memory.editor.optional')}</Badge>
          </dd>
          <dd class="flex min-h-7 min-w-0 flex-col justify-center gap-2">
            {#if field.values.length > 0}
              <!-- Plain monospace words, so the badges stay the only boxes on the page. -->
              <ul class="flex flex-wrap items-center gap-x-2 gap-y-1 font-mono text-sm text-fg">
                {#each field.values as value, index (value)}
                  <li class="flex items-center gap-2">
                    {#if index > 0}<span class="text-fg-3" aria-hidden="true">/</span>{/if}
                    {value}
                  </li>
                {/each}
              </ul>
            {/if}
            <p class="text-sm text-fg-2">{field.text}</p>
          </dd>
        </div>
      {/each}
    </dl>
  </section>

  <section class="flex flex-col gap-4" aria-labelledby="notes-import-example">
    <div class="flex flex-wrap items-end justify-between gap-3">
      <div class="flex flex-col gap-3">
        <h3 id="notes-import-example" class={HEADING}>{t('memory.import.start.example')}</h3>
        <SegmentedControl options={formats} bind:value={format} label={t('memory.import.start.format')} />
      </div>

      <Button loading={flow.savingTemplate} onclick={() => void flow.saveTemplate(format)}>
        {#snippet leading()}<FileDown size={ICON.sm} aria-hidden="true" />{/snippet}
        {t('memory.import.template.save')}
      </Button>
    </div>

    <div class="flex flex-col gap-2">
      <CodeBlock code={IMPORT_EXAMPLES[format]} lang={format} />

      <p class="flex items-center gap-2 text-sm text-fg-3">
        <Info size={ICON.sm} class="shrink-0" aria-hidden="true" />
        {t('memory.import.start.limits')}
      </p>
    </div>
  </section>
</div>
