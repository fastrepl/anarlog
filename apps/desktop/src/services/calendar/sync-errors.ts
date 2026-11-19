import { create } from "zustand";

export type CalendarSyncIssue =
  | "outlook_mailbox_unavailable"
  | "reconnect_required"
  | "sync_failed";

export const useCalendarSyncIssues = create<
  Partial<Record<string, CalendarSyncIssue>>
>(() => ({}));

export function connectionKey(provider: string, connectionId: string): string {
  return `${provider}:${connectionId}`;
}

export function calendarSyncIssue(error: unknown): CalendarSyncIssue {
  return error === "outlook_mailbox_unavailable" ||
    error === "reconnect_required"
    ? error
    : "sync_failed";
}

export function setCalendarSyncIssue(key: string, issue?: CalendarSyncIssue) {
  const current = useCalendarSyncIssues.getState();
  if (current[key] === issue) return;
  const next = { ...current };
  if (issue) next[key] = issue;
  else delete next[key];
  useCalendarSyncIssues.setState(next, true);
}
