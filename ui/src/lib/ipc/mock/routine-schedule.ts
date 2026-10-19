import type { RoutineWhen } from '../bindings/RoutineWhen';
import type { SchedulePreview } from '../bindings/SchedulePreview';

/** How many days ahead the preview looks for a slot before it calls a schedule one that never fires. */
const HORIZON_DAYS = 366;

const MONTHS = ['jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec'];
const WEEKDAYS = ['sun', 'mon', 'tue', 'wed', 'thu', 'fri', 'sat'];

/** One field of a cron expression: the values it allows, and whether it starts with `*` (which makes the two day fields combine with AND). */
interface CronField {
  allowed: Set<number>;
  star: boolean;
}

class CronProblem extends Error {}

function value(text: string, min: number, max: number, names: readonly string[], offset: number): number {
  const named = names.indexOf(text.toLowerCase());
  const number = named >= 0 ? named + offset : /^\d{1,2}$/.test(text) ? Number(text) : Number.NaN;

  if (Number.isNaN(number) || number < min || number > max) {
    throw new CronProblem(`"${text}" is not a value from ${min} to ${max}.`);
  }

  return number;
}

function parseItem(item: string, min: number, max: number, names: readonly string[], offset: number): number[] {
  const [base, step, ...rest] = item.split('/');

  if (rest.length > 0 || base === '') {
    throw new CronProblem(`"${item}" is not a valid item.`);
  }

  const [from, to] = base === '*' ? [min, max] : base.includes('-') ? base.split('-').map((part) => value(part, min, max, names, offset)) : [value(base, min, max, names, offset), null];

  if (to !== null && to < from) {
    throw new CronProblem(`"${base}" is a range that runs backwards.`);
  }

  if (step !== undefined && (to === null || !/^\d{1,2}$/.test(step) || Number(step) < 1)) {
    throw new CronProblem(`"${item}" has a step that does not fit.`);
  }

  const size = Number(step ?? 1);
  const last = to ?? from;

  return Array.from({ length: Math.floor((last - from) / size) + 1 }, (_, index) => from + index * size);
}

function parseField(text: string, min: number, max: number, names: readonly string[] = [], offset = 0): CronField {
  return { allowed: new Set(text.split(',').flatMap((item) => parseItem(item, min, max, names, offset))), star: text.startsWith('*') };
}

/** A five-field cron expression as the format reads it, or why it is not one. */
function parseCron(expression: string): { minute: CronField; hour: CronField; day: CronField; month: CronField; weekday: CronField } {
  const fields = expression.split(' ');

  if (fields.length !== 5 || fields.some((field) => field === '')) {
    throw new CronProblem('A cron expression has exactly five fields separated by single spaces.');
  }

  const weekday = parseField(fields[4], 0, 7, WEEKDAYS);

  if (weekday.allowed.has(7)) {
    weekday.allowed.add(0);
  }

  return {
    minute: parseField(fields[0], 0, 59),
    hour: parseField(fields[1], 0, 23),
    day: parseField(fields[2], 1, 31),
    month: parseField(fields[3], 1, 12, MONTHS, 1),
    weekday,
  };
}

/** Whether a calendar day is one a cron expression fires on: the two day fields combine with OR when neither starts with `*`. */
function dayMatches(cron: ReturnType<typeof parseCron>, date: Date): boolean {
  if (!cron.month.allowed.has(date.getMonth() + 1)) {
    return false;
  }

  const byDay = cron.day.allowed.has(date.getDate());
  const byWeekday = cron.weekday.allowed.has(date.getDay());

  return cron.day.star || cron.weekday.star ? byDay && byWeekday : byDay || byWeekday;
}

function atOf(text: string): { hour: number; minute: number } {
  const [hour, minute] = text.split(':').map(Number);

  return { hour, minute };
}

/** The minutes of a day, as `[hour, minute]`, a schedule fires at. */
function timesOf(when: RoutineWhen, cron: ReturnType<typeof parseCron> | null): [number, number][] {
  switch (when.type) {
    case 'daily':
    case 'weekdays': {
      const { hour, minute } = atOf(when.at);

      return [[hour, minute]];
    }
    case 'everyMinutes':
    case 'everyHours': {
      const step = when.type === 'everyMinutes' ? when.every : when.every * 60;

      return Array.from({ length: Math.ceil(1440 / step) }, (_, index) => [Math.floor((index * step) / 60), (index * step) % 60]);
    }
    case 'cron':
      return [...(cron?.hour.allowed ?? [])].flatMap((hour) => [...(cron?.minute.allowed ?? [])].map((minute): [number, number] => [hour, minute]));
    case 'appStart':
      return [];
  }
}

function runsOn(when: RoutineWhen, cron: ReturnType<typeof parseCron> | null, date: Date): boolean {
  if (when.type === 'weekdays') {
    return date.getDay() >= 1 && date.getDay() <= 5;
  }

  return when.type === 'cron' && cron ? dayMatches(cron, date) : true;
}

/** The next runs of a schedule after `from`, in this machine's time zone, or why it is not valid. */
export function previewSchedule(when: RoutineWhen, from: Date = new Date(), count = 5): SchedulePreview {
  let cron: ReturnType<typeof parseCron> | null = null;

  if (when.type === 'cron') {
    try {
      cron = parseCron(when.expression.trim());
    } catch (error) {
      return { next: [], problem: error instanceof CronProblem ? error.message : 'The expression is not valid.' };
    }
  }

  const times = timesOf(when, cron).sort(([ah, am], [bh, bm]) => ah * 60 + am - (bh * 60 + bm));
  const next: string[] = [];

  for (let offset = 0; offset < HORIZON_DAYS && next.length < count; offset += 1) {
    const day = new Date(from.getFullYear(), from.getMonth(), from.getDate() + offset);

    if (!runsOn(when, cron, day)) {
      continue;
    }

    for (const [hour, minute] of times) {
      const slot = new Date(day.getFullYear(), day.getMonth(), day.getDate(), hour, minute);

      if (slot.getTime() > from.getTime() && next.length < count) {
        next.push(slot.toISOString().replace(/\.\d+Z$/, 'Z'));
      }
    }
  }

  if (when.type !== 'appStart' && next.length === 0) {
    return { next, problem: 'This schedule never runs.' };
  }

  return { next, problem: null };
}
