import { type Tab, useTabs } from ".";
import { computeHistoryFlags, type TabHistory } from "./navigation";

import { id } from "~/shared/utils";

type SessionTab = Extract<Tab, { type: "sessions" }>;
type ContactsTab = Extract<Tab, { type: "contacts" }>;
type SettingsTab = Extract<Tab, { type: "settings" }>;

type SessionOverrides = Partial<Omit<SessionTab, "type" | "state">> & {
  state?: Partial<SessionTab["state"]>;
};

type ContactsOverrides = Partial<Omit<ContactsTab, "type" | "state">> & {
  state?: Partial<ContactsTab["state"]>;
};

type SettingsOverrides = Partial<Omit<SettingsTab, "type" | "state">> & {
  state?: Partial<SettingsTab["state"]>;
};

export const createSessionTab = (
  overrides: SessionOverrides = {},
): SessionTab => ({
  type: "sessions",
  id: overrides.id ?? id(),
  active: overrides.active ?? false,
  pinned: overrides.pinned ?? false,
  slotId: id(),
  state: {
    view: null,
    autoStart: null,
    ...overrides.state,
  },
});

export const createContactsTab = (
  overrides: ContactsOverrides = {},
): ContactsTab => ({
  type: "contacts",
  active: overrides.active ?? false,
  pinned: overrides.pinned ?? false,
  slotId: id(),
  state: {
    selected: null,
    ...overrides.state,
  },
});

export const createSettingsTab = (
  overrides: SettingsOverrides = {},
): SettingsTab => ({
  type: "settings",
  active: overrides.active ?? false,
  pinned: overrides.pinned ?? false,
  slotId: overrides.slotId ?? id(),
  state: {
    tab: "app",
    ...overrides.state,
  },
});

type TabsStore = ReturnType<typeof useTabs.getState>;
type TabsStateSlice = Pick<
  TabsStore,
  | "currentTab"
  | "tabs"
  | "history"
  | "canGoBack"
  | "canGoNext"
  | "onClose"
  | "onEmpty"
  | "closedTabs"
  | "chatMode"
>;

const createDefaultTabsState = (): TabsStateSlice => ({
  currentTab: null,
  tabs: [],
  history: new Map(),
  canGoBack: false,
  canGoNext: false,
  onClose: null,
  onEmpty: null,
  closedTabs: [],
  chatMode: "FloatingClosed",
});

export const seedTabsStore = (
  overrides: Partial<TabsStateSlice> = {},
): void => {
  const state = { ...createDefaultTabsState(), ...overrides };
  useTabs.setState(() => state);
  const flags = computeHistoryFlags(state.history, state.currentTab);
  useTabs.setState(() => flags);
};

export const resetTabsStore = (): void => {
  seedTabsStore();
};

type HistoryEntry = {
  slotId?: string;
  stack: Tab[];
  currentIndex?: number;
};

export const createHistory = (
  entries: HistoryEntry[],
): Map<string, TabHistory> => {
  const history = new Map<string, TabHistory>();

  entries.forEach(({ slotId, stack, currentIndex }) => {
    const key = slotId ?? (stack.length > 0 ? stack[0].slotId : "default");
    history.set(key, {
      stack,
      currentIndex: currentIndex ?? stack.length - 1,
    });
  });

  return history;
};
