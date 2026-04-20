export type StubTabType = "settings" | "folders" | "contacts" | "calendar";

export type Tab =
  | { type: "sessions"; id: string }
  | { type: "daily-summary"; date: string }
  | { type: StubTabType };

export function uniqueIdFromTab(tab: Tab): string {
  switch (tab.type) {
    case "sessions":
      return `sessions-${tab.id}`;
    case "daily-summary":
      return `daily-summary-${tab.date}`;
    case "settings":
      return "settings";
    case "folders":
      return "folders";
    case "contacts":
      return "contacts";
    case "calendar":
      return "calendar";
  }
}

export function getStubTabLabel(type: StubTabType): string {
  switch (type) {
    case "settings":
      return "Settings";
    case "folders":
      return "Folders";
    case "contacts":
      return "Contacts";
    case "calendar":
      return "Calendar";
  }
}

export function getDailySummaryLabel(date: string): string {
  return `Daily Summary · ${date}`;
}

// Single entry point for "what should I call this tab?" used by the tab
// strip and the "not ported yet" placeholder. Session titles come from the
// DB, so the caller threads them in via `sessionTitle`.
export function getTabLabel(tab: Tab, sessionTitle?: string | null): string {
  switch (tab.type) {
    case "sessions":
      return sessionTitle || "Untitled session";
    case "daily-summary":
      return getDailySummaryLabel(tab.date);
    default:
      return getStubTabLabel(tab.type);
  }
}
