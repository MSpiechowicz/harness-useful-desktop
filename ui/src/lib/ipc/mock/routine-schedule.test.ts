import { describe, expect, it } from 'vitest';

import { previewSchedule } from './routine-schedule';

/** A moment in this machine's time zone, so the expectations hold wherever the tests run. */
const FROM = new Date(2026, 9, 19, 8, 30);

function local(iso: string): string {
  const date = new Date(iso);

  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')} ${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`;
}

function next(when: Parameters<typeof previewSchedule>[0], count = 3): string[] {
  return previewSchedule(when, FROM, count).next.map(local);
}

describe('the schedule preview of the mock backend', () => {
  it('lists the next days at a time of the day, and the working days only', () => {
    expect(next({ type: 'daily', at: '09:00' })).toEqual(['2026-10-19 09:00', '2026-10-20 09:00', '2026-10-21 09:00']);
    // Monday 19 October 2026 morning, so Friday is the fourth working day and the weekend is skipped.
    expect(next({ type: 'weekdays', at: '08:00' }, 5)).toEqual(['2026-10-20 08:00', '2026-10-21 08:00', '2026-10-22 08:00', '2026-10-23 08:00', '2026-10-26 08:00']);
  });

  it('counts every N minutes and hours from the start of each day', () => {
    expect(next({ type: 'everyMinutes', every: 45 })).toEqual(['2026-10-19 09:00', '2026-10-19 09:45', '2026-10-19 10:30']);
    expect(next({ type: 'everyHours', every: 5 }, 4)).toEqual(['2026-10-19 10:00', '2026-10-19 15:00', '2026-10-19 20:00', '2026-10-20 00:00']);
  });

  it('reads a cron expression with lists, ranges, steps and names', () => {
    expect(next({ type: 'cron', expression: '*/20 9-10 * * mon-fri' }, 4)).toEqual(['2026-10-19 09:00', '2026-10-19 09:20', '2026-10-19 09:40', '2026-10-19 10:00']);
    expect(next({ type: 'cron', expression: '0 8 1,15 * *' }, 2)).toEqual(['2026-11-01 08:00', '2026-11-15 08:00']);
    expect(next({ type: 'cron', expression: '0 12 * * sun' }, 1)).toEqual(['2026-10-25 12:00']);
  });

  it('lets either day field match when both are restricted, and both when one starts with a star', () => {
    // The 20th or a Wednesday.
    expect(next({ type: 'cron', expression: '0 7 20 * wed' }, 3)).toEqual(['2026-10-20 07:00', '2026-10-21 07:00', '2026-10-28 07:00']);
    // An odd day of the month that is also a Monday: the 19th is past 07:00 already, the 26th and 2 November are even.
    expect(next({ type: 'cron', expression: '0 7 */2 * mon' }, 1)).toEqual(['2026-11-09 07:00']);
  });

  it('says why an expression is not one, and that a schedule that can never match never runs', () => {
    expect(previewSchedule({ type: 'cron', expression: '0 9 * *' }, FROM)).toMatchObject({ next: [], problem: expect.stringContaining('five fields') });
    expect(previewSchedule({ type: 'cron', expression: '61 9 * * *' }, FROM).problem).toContain('"61"');
    expect(previewSchedule({ type: 'cron', expression: '30-10 * * * *' }, FROM).problem).toContain('backwards');
    expect(previewSchedule({ type: 'cron', expression: '5/10 * * * *' }, FROM).problem).toContain('step');
    expect(previewSchedule({ type: 'cron', expression: '0 0 31 2 *' }, FROM)).toEqual({ next: [], problem: 'This schedule never runs.' });
  });

  it('has no next run for the start of the app, and no problem', () => {
    expect(previewSchedule({ type: 'appStart' }, FROM)).toEqual({ next: [], problem: null });
  });
});
