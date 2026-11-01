import {invalidInput} from './human-input.js';

export const timeWords = ['today', 'yesterday', 'week', 'last-week', 'month'] as const;
export type TimeWord = typeof timeWords[number];
export function isTimeWord(value: string | undefined): value is TimeWord {return timeWords.some(word => word === value);}
export function validateTimezone(zone: string): string {
  try {new Intl.DateTimeFormat('en', {timeZone: zone});} catch {invalidInput('--timezone '+zone);}
  return zone;
}
export function shiftDate(date: string, days: number): string {
  const d = new Date(date+'T00:00:00Z'); d.setUTCDate(d.getUTCDate()+days);
  return d.toISOString().slice(0,10);
}
export function localDate(now: Date, timezone: string): string {
  const parts = new Intl.DateTimeFormat('en', {timeZone: validateTimezone(timezone), year:'numeric',month:'2-digit',day:'2-digit'}).formatToParts(now);
  return ['year','month','day'].map(key => parts.find(p => p.type === key)!.value).join('-');
}
export function dateWindow(options: {since?: string | null; until?: string | null; timezone?: string | null; allTime?: boolean | null; undated?: boolean | null}, fallback: TimeWord | 'recent' | 'compare', word?: TimeWord, now = new Date()) {
  const timezone = validateTimezone(options.timezone ?? Intl.DateTimeFormat().resolvedOptions().timeZone);
  if (word && (options.since || options.until || options.allTime || options.undated)) invalidInput('date range');
  if (Boolean(options.since) !== Boolean(options.until) || options.allTime && (options.since || options.undated)) invalidInput('date range');
  for (const date of [options.since, options.until]) if (date && (!/^\d{4}-\d{2}-\d{2}$/.test(date) || !Number.isFinite(Date.parse(date)) || new Date(date).toISOString().slice(0,10) !== date)) invalidInput(date);
  if (options.since && options.until && options.since >= options.until) invalidInput('date range');
  if (options.since || options.allTime || options.undated) return {...options, timezone};
  const today = localDate(now, timezone), day = new Date(today+'T00:00:00Z').getUTCDay();
  const monday = shiftDate(today, -((day+6)%7));
  let since = today, until = shiftDate(today, 1);
  switch (word ?? fallback) {
    case 'yesterday': since = shiftDate(today,-1); until = today; break;
    case 'week': since = monday; break;
    case 'last-week': since = shiftDate(monday,-7); until = monday; break;
    case 'month': since = today.slice(0,8)+'01'; break;
    case 'recent': since = shiftDate(today,-29); break;
    case 'compare': since = shiftDate(monday,-7); until = monday; break;
  }
  return {since, until, timezone};
}
