import "./test-matchers";

import { beforeEach, describe, expect, test, vi } from "vitest";

import { type Tab, useTabs } from ".";
import {
  createContactsTab,
  createSessionTab,
  resetTabsStore,
} from "./test-utils";

const isSessionsTab = (tab: Tab): tab is Extract<Tab, { type: "sessions" }> =>
  tab.type === "sessions";

describe("Basic Tab Actions", () => {
  beforeEach(() => {
    resetTabsStore();
  });

  test("openNew builds tab list with last tab active", () => {
    const session1 = createSessionTab({ active: false });
    const session2 = createSessionTab({
      active: false,
      state: { view: { type: "enhanced", id: "note-1" } },
    });
    const contacts = createContactsTab({ active: false });

    useTabs.getState().openNew(session1);
    useTabs.getState().openNew(session2);
    useTabs.getState().openNew(contacts);

    expect(useTabs.getState()).toHaveCurrentTab({ type: "contacts" });
    expect(useTabs.getState()).toMatchTabsInOrder([
      { id: session1.id, active: false, type: "sessions" },
      { id: session2.id, active: false, type: "sessions" },
      { type: "contacts", active: true },
    ]);
    expect(useTabs.getState()).toHaveHistoryLength(1);
    expect(useTabs.getState()).toHaveNavigationState({
      canGoBack: false,
      canGoNext: false,
    });
  });

  test("openCurrent switches to existing tab instead of replacing active", () => {
    const session1 = createSessionTab({ active: false });
    const session2 = createSessionTab({ active: false });
    const session3 = createSessionTab({ active: false });
    useTabs.getState().openNew(session1);
    useTabs.getState().openNew(session2);
    useTabs.getState().openNew(session3);

    const duplicateOfSession1 = createSessionTab({
      id: session1.id,
      active: false,
    });
    useTabs.getState().openCurrent(duplicateOfSession1);

    expect(useTabs.getState()).toMatchTabsInOrder([
      { id: session1.id, active: true },
      { id: session2.id, active: false },
      { id: session3.id, active: false },
    ]);
    expect(useTabs.getState()).toHaveHistoryLength(1);
    expect(useTabs.getState()).toHaveNavigationState({
      canGoBack: false,
      canGoNext: false,
    });
  });

  test("openCurrent closes existing active tab via lifecycle handlers", () => {
    const handler = vi.fn();
    const active = createSessionTab({ id: "first", active: false });
    useTabs.getState().registerOnClose(handler);
    useTabs.getState().openCurrent(active);

    const next = createSessionTab({ id: "second", active: false });
    useTabs.getState().openCurrent(next);

    expect(handler).toHaveBeenCalledTimes(1);
    expect(handler).toHaveBeenCalledWith(
      expect.objectContaining({ id: "first" }),
    );
  });

  test("openNew is idempotent - switches to existing tab instead of duplicating", () => {
    const session1 = createSessionTab({ id: "tab1", active: false });
    const session2 = createSessionTab({ id: "tab2", active: false });
    useTabs.getState().openNew(session1);
    useTabs.getState().openNew(session2);

    useTabs.getState().openNew(createSessionTab({ id: "tab1", active: false }));

    const state = useTabs.getState();
    expect(state).toMatchTabsInOrder([
      { id: "tab1", active: true },
      { id: "tab2", active: false },
    ]);
    expect(state).toHaveHistoryLength(1);
  });

  test("select toggles active flag without changing history", () => {
    const tabA = createSessionTab({ active: true });
    const tabB = createSessionTab({ active: false });
    useTabs.getState().openNew(tabA);
    useTabs.getState().openNew(tabB);

    useTabs.getState().select(tabA);

    const state = useTabs.getState();
    if (!state.currentTab || !isSessionsTab(state.currentTab)) {
      throw new Error("expected current tab to be a sessions tab");
    }
    expect(state.currentTab.id).toBe(tabA.id);
    const target = state.tabs.find((t) => isSessionsTab(t) && t.id === tabA.id);
    expect(target?.active).toBe(true);
    expect(useTabs.getState()).toMatchTabsInOrder([
      { id: tabA.id, active: true },
      { id: tabB.id, active: false },
    ]);
    expect(useTabs.getState()).toHaveHistoryLength(1);
  });

  test("close removes tab, picks fallback active, updates history", () => {
    const active = createSessionTab({ active: true });
    const next = createSessionTab({ active: false });
    useTabs.getState().openNew(active);
    useTabs.getState().openNew(next);

    useTabs.getState().close(active);

    expect(useTabs.getState()).toMatchTabsInOrder([
      { id: next.id, active: true },
    ]);
    expect(useTabs.getState()).toHaveCurrentTab({ id: next.id });
    expect(useTabs.getState().history.size).toBe(1);
    expect(useTabs.getState()).toHaveNavigationState({
      canGoBack: false,
      canGoNext: false,
    });
  });

  test("close last tab empties store", () => {
    const only = createSessionTab({ active: true });
    useTabs.getState().openNew(only);

    useTabs.getState().close(only);

    expect(useTabs.getState().tabs).toHaveLength(0);
    expect(useTabs.getState().currentTab).toBeNull();
    expect(useTabs.getState()).toHaveNavigationState({
      canGoBack: false,
      canGoNext: false,
    });
  });

  test("reorder keeps current tab and flags consistent", () => {
    const active = createSessionTab({ active: true });
    const other = createSessionTab({ active: false });
    useTabs.getState().openNew(active);
    useTabs.getState().openNew(other);

    useTabs.getState().reorder([other, { ...active, active: true }]);

    expect(useTabs.getState()).toMatchTabsInOrder([
      { id: other.id, active: false },
      { id: active.id, active: true },
    ]);
    expect(useTabs.getState()).toHaveCurrentTab(active);
    expect(useTabs.getState()).toHaveNavigationState({
      canGoBack: false,
      canGoNext: false,
    });
  });

  test("closeOthers keeps selected tab and notifies closures", () => {
    const session1 = createSessionTab({ active: true });
    const session2 = createSessionTab({ active: false });
    const session3 = createSessionTab({ active: false });
    const handler = vi.fn();

    useTabs.getState().openNew(session1);
    useTabs.getState().openNew(session2);
    useTabs.getState().openNew(session3);
    useTabs.getState().select(session2);
    useTabs.getState().registerOnClose(handler);

    useTabs.getState().closeOthers(session2);

    const state = useTabs.getState();
    expect(state.tabs).toHaveLength(1);
    expect(state).toHaveCurrentTab({ id: session2.id });
    expect(state).toHaveNavigationState({
      canGoBack: false,
      canGoNext: false,
    });
    expect(state.history.size).toBe(1);
    expect(handler).toHaveBeenCalledTimes(2);
    expect(handler).toHaveBeenCalledWith(
      expect.objectContaining({ id: session1.id }),
    );
    expect(handler).toHaveBeenCalledWith(
      expect.objectContaining({ id: session3.id }),
    );
  });

  test("closeAll clears store state and notifies handlers", () => {
    const first = createSessionTab({ active: true });
    const second = createContactsTab({ active: false });
    const onClose = vi.fn();
    const onEmpty = vi.fn();

    useTabs.getState().openNew(first);
    useTabs.getState().openNew(second);
    useTabs.getState().registerOnClose(onClose);
    useTabs.getState().registerOnEmpty(onEmpty);

    useTabs.getState().closeAll();

    const state = useTabs.getState();
    expect(state.tabs).toHaveLength(0);
    expect(state.currentTab).toBeNull();
    expect(state.history.size).toBe(0);
    expect(state).toHaveNavigationState({
      canGoBack: false,
      canGoNext: false,
    });
    expect(onClose).toHaveBeenCalledTimes(2);
    expect(onClose).toHaveBeenCalledWith(
      expect.objectContaining({ id: first.id }),
    );
    expect(onClose).toHaveBeenCalledWith(
      expect.objectContaining({ type: "contacts" }),
    );
    expect(onEmpty).toHaveBeenCalledTimes(1);
  });
});
