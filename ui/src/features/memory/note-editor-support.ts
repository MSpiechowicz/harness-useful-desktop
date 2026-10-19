import { fireEvent, screen, within } from '@testing-library/svelte';

import { noteRow, showNotes } from './memory-test-support';

export const TAKEN_TITLE = 'Notes are read through the Memory CLI, never its database';

/** Opens the editor on a new note from the primary action of the header. */
export async function openEditor(): Promise<HTMLElement> {
  await showNotes();
  await fireEvent.click(await screen.findByRole('button', { name: 'Add note' }));

  return screen.findByRole('dialog', { name: 'Add a note' });
}

/** Types `text` into a chip field of the editor, the way a keystroke does. */
export async function type(dialog: HTMLElement, label: string, text: string): Promise<HTMLElement> {
  const field = within(dialog).getByLabelText(label);

  await fireEvent.input(field, { target: { value: text } });

  return field;
}

/** The chips of a field, by the buttons that remove them. */
export const chipsOf = (dialog: HTMLElement, kind: 'tag' | 'entity') =>
  within(dialog)
    .queryAllByRole('button', { name: new RegExp(`^Remove ${kind} `) })
    .map((button) => button.getAttribute('aria-label')?.replace(`Remove ${kind} `, ''));

export async function fill(dialog: HTMLElement, fields: { title?: string; body?: string; tags?: string }): Promise<void> {
  if (fields.title !== undefined) {
    await fireEvent.input(within(dialog).getByLabelText('Title'), { target: { value: fields.title } });
  }

  if (fields.body !== undefined) {
    await fireEvent.input(within(dialog).getByLabelText('Content'), { target: { value: fields.body } });
  }

  if (fields.tags !== undefined) {
    await fireEvent.input(within(dialog).getByLabelText('Tags'), { target: { value: fields.tags } });
  }
}

/** Opens the editor on a note that exists, from the Edit button of its panel. */
export async function openEdit(id: number, title: string): Promise<HTMLElement> {
  await showNotes();
  await fireEvent.click(noteRow(id) as HTMLElement);
  const panel = await screen.findByRole('region', { name: title });
  await within(panel).findByText(/WebKitGTK renders into a DMA buffer/);
  await fireEvent.click(within(panel).getByRole('button', { name: 'Edit' }));

  return screen.findByRole('dialog', { name: 'Edit note' });
}
