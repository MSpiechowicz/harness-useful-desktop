import { timeOfDay } from '$lib/chat/subagent-time';
import { formatDateTime } from '$lib/utils/format';

/** When a message was sent: the time of day for today, the date and time for an earlier day. Empty when the time is unknown. */
export function sentAt(timestamp: number | null | undefined, now: Date = new Date()): string {
  if (timestamp === null || timestamp === undefined || Number.isNaN(new Date(timestamp).getTime())) {
    return '';
  }

  return new Date(timestamp).toDateString() === now.toDateString() ? timeOfDay(timestamp) : formatDateTime(timestamp);
}
