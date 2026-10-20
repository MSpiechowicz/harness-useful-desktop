import { fireEvent, screen, within } from '@testing-library/svelte';

import { showMemory } from './memory-test-support';

export const BODY = '#!/usr/bin/env bash\necho todo';

/** Opens the editor on a new script from the primary action of the header. */
export async function openEditor(): Promise<HTMLElement> {
  await showMemory();
  await fireEvent.click(await screen.findByRole('button', { name: 'Add script' }));

  return screen.findByRole('dialog', { name: 'Add a script' });
}

/** Types into the fields of the script editor, the way keystrokes do. */
export async function fill(dialog: HTMLElement, fields: { name?: string; description?: string; body?: string; timeout?: string }): Promise<void> {
  const labels = { name: 'Name', description: 'Description', body: 'Code', timeout: 'Time limit' } as const;

  for (const [field, value] of Object.entries(fields)) {
    await fireEvent.input(within(dialog).getByLabelText(labels[field as keyof typeof labels]), { target: { value } });
  }
}
