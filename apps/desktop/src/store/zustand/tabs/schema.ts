import type {
  ChangelogState,
  ContactsSelection,
  ContactsState,
  EditorView,
  SessionsState,
  TabInput as WindowsTabInput,
  TemplatesState,
} from "@hypr/plugin-windows";

export type {
  ChangelogState,
  ContactsSelection,
  ContactsState,
  EditorView,
  SessionsState,
  TemplatesState,
};

export type TabInput = Exclude<
  WindowsTabInput,
  { type: "extension" } | { type: "extensions" }
>;

export const isTabInputSupported = (tab: WindowsTabInput): tab is TabInput => {
  return tab.type !== "extension" && tab.type !== "extensions";
};

export type SettingsTab =
  | "account"
  | "app"
  | "notifications"
  | "permissions"
  | "transcription"
  | "intelligence";

export const normalizeSettingsTab = (
  tab: string | null | undefined,
): Exclude<SettingsTab, "account"> => {
  switch (tab) {
    case "app":
    case "notifications":
    case "permissions":
    case "transcription":
    case "intelligence":
      return tab;
    case "account":
    default:
      return "app";
  }
};

export type SettingsState = {
  tab: SettingsTab | null;
};

export type DailySummaryState = {
  activeTab: "timeline" | "raw" | null;
};

export const isEnhancedView = (
  view: EditorView,
): view is { type: "enhanced"; id: string } => view.type === "enhanced";
export const isRawView = (view: EditorView): view is { type: "raw" } =>
  view.type === "raw";

type BaseTab = {
  active: boolean;
  slotId: string;
  pinned: boolean;
};

export type Tab =
  | (BaseTab & {
      type: "sessions";
      id: string;
      state: SessionsState;
    })
  | (BaseTab & {
      type: "contacts";
      state: ContactsState;
    })
  | (BaseTab & {
      type: "templates";
      state: TemplatesState;
    })
  | (BaseTab & {
      type: "humans";
      id: string;
    })
  | (BaseTab & { type: "organizations"; id: string })
  | (BaseTab & { type: "folders"; id: string | null })
  | (BaseTab & { type: "empty" })
  | (BaseTab & { type: "calendar" })
  | (BaseTab & {
      type: "changelog";
      state: ChangelogState;
    })
  | (BaseTab & { type: "settings"; state: SettingsState })
  | (BaseTab & { type: "onboarding" })
  | (BaseTab & { type: "edit"; requestId: string })
  | (BaseTab & {
      type: "task";
      id: string;
      resources: TaskResource[];
    })
  | (BaseTab & {
      type: "daily_summary";
      id: string;
      state: DailySummaryState;
    });

export type TaskResource =
  | { type: "github_issue"; owner: string; repo: string; number: number }
  | { type: "github_pr"; owner: string; repo: string; number: number };

export const getDefaultState = (tab: TabInput): Tab => {
  const base = { active: false, slotId: "", pinned: false };

  switch (tab.type) {
    case "sessions":
      return {
        ...base,
        type: "sessions",
        id: tab.id,
        state: tab.state ?? { view: null, autoStart: null },
      };
    case "contacts":
      return {
        ...base,
        type: "contacts",
        state: tab.state ?? {
          selected: null,
        },
      };
    case "templates":
      return {
        ...base,
        type: "templates",
        state: tab.state ?? {
          showHomepage: false,
          isWebMode: true,
          selectedMineId: null,
          selectedWebIndex: null,
        },
      };
    case "humans":
      return { ...base, type: "humans", id: tab.id };
    case "organizations":
      return { ...base, type: "organizations", id: tab.id };
    case "folders":
      return { ...base, type: "folders", id: tab.id };
    case "empty":
      return { ...base, type: "empty" };
    case "calendar":
      return { ...base, type: "calendar" };
    case "changelog":
      return {
        ...base,
        type: "changelog",
        state: tab.state,
      };
    case "settings": {
      const subtab = tab.state?.tab as string | null | undefined;
      if (subtab === "calendar") {
        return { ...base, type: "calendar" };
      }
      const settingsTab =
        subtab === "account" ? "account" : normalizeSettingsTab(subtab);
      return {
        ...base,
        type: "settings",
        state: { tab: settingsTab },
      };
    }
    case "onboarding":
      return { ...base, type: "onboarding" };
    case "edit":
      return { ...base, type: "edit", requestId: tab.requestId };
    default:
      const _exhaustive: never = tab;
      return _exhaustive;
  }
};

export const uniqueIdfromTab = (tab: Tab): string => {
  switch (tab.type) {
    case "sessions":
      return `sessions-${tab.id}`;
    case "humans":
      return `humans-${tab.id}`;
    case "organizations":
      return `organizations-${tab.id}`;
    case "contacts":
      return `contacts`;
    case "templates":
      return `templates`;
    case "folders":
      return `folders-${tab.id ?? "all"}`;
    case "empty":
      return `empty-${tab.slotId}`;
    case "calendar":
      return `calendar`;
    case "changelog":
      return "changelog";
    case "settings":
      return `settings`;
    case "onboarding":
      return `onboarding`;
    case "edit":
      return `edit-${tab.requestId}`;
    case "task":
      return `task-${tab.id}`;
    case "daily_summary":
      return `daily_summary-${tab.id}`;
  }
};

export const isSameTab = (a: Tab, b: Tab) => {
  return uniqueIdfromTab(a) === uniqueIdfromTab(b);
};
