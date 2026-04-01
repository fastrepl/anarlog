import type {
  ChangelogState,
  ChatShortcutsState,
  ChatState,
  ContactsSelection,
  ContactsState,
  EditorView,
  ExtensionsState,
  PromptsState,
  SessionsState,
  TabInput,
  TemplatesState,
} from "@hypr/plugin-windows";

export type {
  ChangelogState,
  ChatShortcutsState,
  ChatState,
  ContactsSelection,
  ContactsState,
  EditorView,
  ExtensionsState,
  PromptsState,
  SessionsState,
  TabInput,
  TemplatesState,
};

export type SettingsTab =
  | "account"
  | "app"
  | "notifications"
  | "calendar"
  | "system"
  | "lab"
  | "transcription"
  | "intelligence"
  | "memory"
  | "todo"
  | "dont-use-this";

export const normalizeSettingsTab = (
  tab: string | null | undefined,
): Exclude<SettingsTab, "account"> => {
  switch (tab) {
    case "app":
    case "notifications":
    case "calendar":
    case "system":
    case "lab":
    case "transcription":
    case "intelligence":
    case "memory":
    case "dont-use-this":
      return tab;
    case "account":
    default:
      return "app";
  }
};

export type SettingsState = {
  tab: SettingsTab | null;
};

export const isEnhancedView = (
  view: EditorView,
): view is { type: "enhanced"; id: string } => view.type === "enhanced";
export const isRawView = (view: EditorView): view is { type: "raw" } =>
  view.type === "raw";
export const isTranscriptView = (
  view: EditorView,
): view is { type: "transcript" } => view.type === "transcript";
export const isAttachmentsView = (
  view: EditorView,
): view is { type: "attachments" } => view.type === "attachments";

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
      type: "prompts";
      state: PromptsState;
    })
  | (BaseTab & {
      type: "chat_shortcuts";
      state: ChatShortcutsState;
    })
  | (BaseTab & {
      type: "humans";
      id: string;
    })
  | (BaseTab & { type: "organizations"; id: string })
  | (BaseTab & { type: "folders"; id: string | null })
  | (BaseTab & { type: "empty" })
  | (BaseTab & {
      type: "extension";
      extensionId: string;
      state: Record<string, unknown>;
    })
  | (BaseTab & {
      type: "extensions";
      state: ExtensionsState;
    })
  | (BaseTab & { type: "calendar" })
  | (BaseTab & {
      type: "changelog";
      state: ChangelogState;
    })
  | (BaseTab & { type: "settings"; state: SettingsState })
  | (BaseTab & {
      type: "chat_support";
      state: ChatState;
    })
  | (BaseTab & { type: "onboarding" })
  | (BaseTab & { type: "daily" })
  | (BaseTab & { type: "edit"; requestId: string });

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
    case "prompts":
      return {
        ...base,
        type: "prompts",
        state: tab.state ?? {
          selectedTask: null,
        },
      };
    case "chat_shortcuts":
      return {
        ...base,
        type: "chat_shortcuts",
        state: tab.state ?? {
          isWebMode: null,
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
    case "extension":
      return {
        ...base,
        type: "extension",
        extensionId: tab.extensionId,
        state: tab.state ?? {},
      };
    case "extensions":
      return {
        ...base,
        type: "extensions",
        state: tab.state ?? { selectedExtension: null },
      };
    case "calendar":
      return { ...base, type: "calendar" };
    case "changelog":
      return {
        ...base,
        type: "changelog",
        state: tab.state,
      };
    case "settings":
      return {
        ...base,
        type: "settings",
        state: { tab: (tab.state?.tab as SettingsTab) ?? "app" },
      };
    case "chat_support":
      return {
        ...base,
        type: "chat_support",
        state: tab.state ?? {
          groupId: null,
          initialMessage: null,
        },
      };
    case "onboarding":
      return { ...base, type: "onboarding" };
    case "daily":
      return { ...base, type: "daily" };
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
    case "prompts":
      return `prompts`;
    case "chat_shortcuts":
      return `chat_shortcuts`;
    case "folders":
      return `folders-${tab.id ?? "all"}`;
    case "empty":
      return `empty-${tab.slotId}`;
    case "extension":
      return `extension-${tab.extensionId}`;
    case "extensions":
      return `extensions`;
    case "calendar":
      return `calendar`;
    case "changelog":
      return "changelog";
    case "settings":
      return `settings`;
    case "chat_support":
      return `chat_support`;
    case "onboarding":
      return `onboarding`;
    case "daily":
      return `daily`;
    case "edit":
      return `edit-${tab.requestId}`;
  }
};

export const isSameTab = (a: Tab, b: Tab) => {
  return uniqueIdfromTab(a) === uniqueIdfromTab(b);
};
