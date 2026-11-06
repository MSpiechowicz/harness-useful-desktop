/** Where a question stands among the questions of one call. */
export interface QuestionStep {
  number: number;
  total: number;
}

export interface QuestionTitle {
  text: string;
  step: QuestionStep | null;
}

// An older OMP without ask dialogs sends a question at a time: `text (2/3)`.
const SUFFIXED = /^([\s\S]+) \((\d+)\/(\d+)\)$/;

function parsed(text: string, number: string, total: string): QuestionTitle | null {
  const step = { number: Number(number), total: Number(total) };

  return step.total > 1 && step.number >= 1 && step.number <= step.total ? { text, step } : null;
}

/** A question's title split into its text and its step. A title with no valid step stays whole. */
export function questionTitle(title: string): QuestionTitle {
  const suffixed = SUFFIXED.exec(title);
  const split = suffixed && parsed(suffixed[1], suffixed[2], suffixed[3]);

  return split ?? { text: title, step: null };
}

const RECOMMENDED = /\s*\(recommended\)\s*$/i;

/** An option's label without the "(Recommended)" the agent wrote into it, and whether it was there. */
export function optionLabel(label: string): { text: string; recommended: boolean } {
  const text = label.replace(RECOMMENDED, '');

  return { text: text === '' ? label : text, recommended: text !== label && text !== '' };
}

/** A question's answers while it is on screen. */
export interface Draft {
  /** The positions of the picked options: two rows can share a label. */
  selected: number[];
  custom: string;
  typing: boolean;
}

/** The answers given to request `id`, where the user was. */
export interface SavedAnswers {
  id: string;
  step: number;
  drafts: Draft[];
}

/** A question as a title and, after its first line, the text to read below it (an agent's material to review). */
export function questionText(question: string): { title: string; body: string | null } {
  const [title, ...rest] = question.trim().split('\n');
  const body = rest.join('\n').trim();

  return { title, body: body === '' ? null : body };
}
