export type StubTabType =
  | "settings"
  | "folders"
  | "contacts"
  | "calendar"
  | "daily-summary";

export type Tab =
  | { type: "sessions"; id: string }
  | { type: "daily-summary"; date: string }
  | { type: "settings" }
  | { type: "folders" }
  | { type: "contacts" }
  | { type: "calendar" };

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

export function getStubTabLabel(type: StubTabType, hint?: string): string {
  switch (type) {
    case "settings":
      return "Settings";
    case "folders":
      return "Folders";
    case "contacts":
      return "Contacts";
    case "calendar":
      return "Calendar";
    case "daily-summary":
      return hint ? `Daily Summary · ${hint}` : "Daily Summary";
  }
}
