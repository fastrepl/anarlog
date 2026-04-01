import "./test-matchers";

import { beforeEach, describe, expect, test } from "vitest";

import { type Tab, useTabs } from ".";
import {
  createContactsTab,
  createSessionTab,
  resetTabsStore,
} from "./test-utils";

describe("State Updater Actions", () => {
  beforeEach(() => {
    resetTabsStore();
  });

  describe("updateSessionTabState", () => {
    test("updates matching session tab and current tab state", () => {
      const tab = createSessionTab({ active: true });
      useTabs.getState().openNew(tab);

      useTabs.getState().updateSessionTabState(tab, {
        ...tab.state,
        view: { type: "enhanced", id: "note-1" },
      });

      const state = useTabs.getState();
      expect(state.tabs[0]).toMatchObject({
        id: tab.id,
        state: { view: { type: "enhanced", id: "note-1" }, autoStart: null },
      });
      expect(useTabs.getState()).toHaveCurrentTab({
        id: tab.id,
        state: { view: { type: "enhanced", id: "note-1" }, autoStart: null },
      });
      expect(useTabs.getState()).toHaveLastHistoryEntry({
        state: { view: { type: "enhanced", id: "note-1" }, autoStart: null },
      });
    });

    test("updates only matching tab instances", () => {
      const tab = createSessionTab({ active: false });
      const active = createSessionTab({ active: true });
      useTabs.getState().openNew(tab);
      useTabs.getState().openNew(active);

      useTabs.getState().updateSessionTabState(tab, {
        ...tab.state,
        view: { type: "enhanced", id: "note-1" },
      });

      const state = useTabs.getState();
      expect(state.tabs[0]).toMatchObject({
        id: tab.id,
        state: { view: { type: "enhanced", id: "note-1" } },
      });
      expect(state.tabs[1]).toMatchObject({
        id: active.id,
        state: { view: null, autoStart: null },
      });
      expect(useTabs.getState()).toHaveLastHistoryEntry({
        id: active.id,
        state: { view: null, autoStart: null },
      });
    });

    test("no-op when tab types mismatch", () => {
      const session = createSessionTab({ active: true });
      const contacts = createContactsTab();
      useTabs.getState().openNew(session);
      useTabs.getState().openNew(contacts);

      useTabs
        .getState()
        .updateSessionTabState(contacts as Tab, { view: "enhanced" } as any);

      const state = useTabs.getState();
      expect(state.tabs[0]).toMatchObject({
        id: session.id,
        state: { view: null, autoStart: null },
      });
      expect(state.tabs[1]).toMatchObject({ type: "contacts" });
    });
  });

  describe("updateContactsTabState", () => {
    const newContactsState = {
      selected: { type: "person" as const, id: "person-1" },
    };

    test("updates contacts tab and current tab state", () => {
      const contacts = createContactsTab({ active: true });
      useTabs.getState().openNew(contacts);

      useTabs.getState().updateContactsTabState(contacts, newContactsState);

      const state = useTabs.getState();
      expect(state.tabs[0]).toMatchObject({ state: newContactsState });
      expect(useTabs.getState()).toHaveCurrentTab({
        state: newContactsState,
      });
      expect(useTabs.getState()).toHaveLastHistoryEntry({
        state: newContactsState,
      });
    });

    test("only matching contacts tab receives update", () => {
      const contacts = createContactsTab({ active: false });
      const session = createSessionTab({ active: true });
      useTabs.getState().openNew(contacts);
      useTabs.getState().openNew(session);

      useTabs.getState().updateContactsTabState(contacts, newContactsState);

      const state = useTabs.getState();
      expect(state.tabs[0]).toMatchObject({ state: newContactsState });
      expect(state.tabs[1]).toMatchObject({
        state: { view: null, autoStart: null },
      });
      expect(useTabs.getState()).toHaveLastHistoryEntry({
        id: session.id,
      });
    });

    test("updates contacts tab state using any contacts instance", () => {
      const contacts = createContactsTab({ active: true });
      useTabs.getState().openNew(contacts);

      const otherInstance = createContactsTab({ active: true });
      useTabs
        .getState()
        .updateContactsTabState(otherInstance, newContactsState);

      const state = useTabs.getState();
      expect(state.tabs[0]).toMatchObject({ state: newContactsState });
      expect(useTabs.getState()).toHaveCurrentTab({
        state: newContactsState,
      });
    });
  });
});
