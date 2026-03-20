import "./test-matchers";

import { beforeEach, describe, expect, test } from "vitest";

import { useTabs } from ".";
import { createSessionTab, resetTabsStore } from "./test-utils";

describe("navigation", () => {
  beforeEach(() => {
    resetTabsStore();
  });

  test("openNew creates new slot with its own history", () => {
    const tab1 = createSessionTab();
    const tab2 = createSessionTab();

    useTabs.getState().openNew(tab1);
    useTabs.getState().openNew(tab2);

    const state = useTabs.getState();
    expect(state.tabs).toHaveLength(2);
    expect(state.history.size).toBe(2);
    expect(state).toHaveCurrentTab({ id: tab2.id });
  });

  test("openCurrent adds to current slot's history", () => {
    const tab1 = createSessionTab();
    const tab2 = createSessionTab();

    useTabs.getState().openNew(tab1);
    useTabs.getState().openCurrent(tab2);

    const state = useTabs.getState();
    expect(state.tabs).toHaveLength(1);
    expect(state.history.size).toBe(1);
    expect(state).toHaveCurrentTab({ id: tab2.id });
    expect(state).toHaveHistoryLength(2);
  });

  test("goBack navigates within slot's history", () => {
    const tab1 = createSessionTab();
    const tab2 = createSessionTab();

    useTabs.getState().openNew(tab1);
    useTabs.getState().openCurrent(tab2);
    expect(useTabs.getState()).toHaveCurrentTab({ id: tab2.id });
    expect(useTabs.getState().canGoBack).toBe(true);

    useTabs.getState().goBack();
    expect(useTabs.getState()).toHaveCurrentTab({ id: tab1.id });
    expect(useTabs.getState().canGoBack).toBe(false);
  });

  test("goNext navigates forward in slot's history", () => {
    const tab1 = createSessionTab();
    const tab2 = createSessionTab();

    useTabs.getState().openNew(tab1);
    useTabs.getState().openCurrent(tab2);
    useTabs.getState().goBack();
    expect(useTabs.getState()).toHaveCurrentTab({ id: tab1.id });
    expect(useTabs.getState().canGoNext).toBe(true);

    useTabs.getState().goNext();
    expect(useTabs.getState()).toHaveCurrentTab({ id: tab2.id });
    expect(useTabs.getState().canGoNext).toBe(false);
  });

  test("multiple openCurrent calls build history stack in slot", () => {
    const tab1 = createSessionTab();
    const tab2 = createSessionTab();
    const tab3 = createSessionTab();

    useTabs.getState().openNew(tab1);
    useTabs.getState().openCurrent(tab2);
    useTabs.getState().openCurrent(tab3);

    expect(useTabs.getState()).toHaveHistoryLength(3);
    expect(useTabs.getState()).toHaveCurrentTab({ id: tab3.id });

    useTabs.getState().goBack();
    expect(useTabs.getState()).toHaveCurrentTab({ id: tab2.id });

    useTabs.getState().goBack();
    expect(useTabs.getState()).toHaveCurrentTab({ id: tab1.id });
  });

  test("each slot maintains independent history", () => {
    const tab1 = createSessionTab();
    const tab2 = createSessionTab();
    const tab3 = createSessionTab();
    const tab4 = createSessionTab();

    useTabs.getState().openNew(tab1);
    useTabs.getState().openCurrent(tab2);
    useTabs.getState().openNew(tab3);
    useTabs.getState().openCurrent(tab4);

    const state = useTabs.getState();
    expect(state.tabs).toHaveLength(2);
    expect(state.history.size).toBe(2);

    const slot1History = Array.from(state.history.values())[0];
    const slot2History = Array.from(state.history.values())[1];

    expect(slot1History.stack).toHaveLength(2);
    expect(slot2History.stack).toHaveLength(2);
  });

  describe("invalidateResource", () => {
    test("removes invalidated tab from history stack", () => {
      const tab1 = createSessionTab();
      const tab2 = createSessionTab();
      const tab3 = createSessionTab();

      useTabs.getState().openNew(tab1);
      useTabs.getState().openCurrent(tab2);
      useTabs.getState().openCurrent(tab3);

      expect(useTabs.getState()).toHaveHistoryLength(3);

      useTabs.getState().invalidateResource("sessions", tab2.id);

      const state = useTabs.getState();
      expect(state).toHaveHistoryLength(2);
      const history = Array.from(state.history.values())[0];
      expect(
        history.stack.some((t) => t.type === "sessions" && t.id === tab2.id),
      ).toBe(false);
      expect(
        history.stack.some((t) => t.type === "sessions" && t.id === tab1.id),
      ).toBe(true);
      expect(
        history.stack.some((t) => t.type === "sessions" && t.id === tab3.id),
      ).toBe(true);
    });

    test("adjusts currentIndex when invalidated tab is before current position", () => {
      const tab1 = createSessionTab();
      const tab2 = createSessionTab();
      const tab3 = createSessionTab();

      useTabs.getState().openNew(tab1);
      useTabs.getState().openCurrent(tab2);
      useTabs.getState().openCurrent(tab3);

      expect(useTabs.getState()).toHaveCurrentTab({ id: tab3.id });

      useTabs.getState().invalidateResource("sessions", tab1.id);

      const state = useTabs.getState();
      expect(state).toHaveHistoryLength(2);
      expect(state).toHaveCurrentTab({ id: tab3.id });
      const history = Array.from(state.history.values())[0];
      expect(history.currentIndex).toBe(1);
    });

    test("removes current active tab when invalidated", () => {
      const tab1 = createSessionTab();
      const tab2 = createSessionTab();

      useTabs.getState().openNew(tab1);
      useTabs.getState().openNew(tab2);

      expect(useTabs.getState()).toHaveCurrentTab({ id: tab2.id });
      expect(useTabs.getState().tabs).toHaveLength(2);

      useTabs.getState().invalidateResource("sessions", tab2.id);

      const state = useTabs.getState();
      expect(state.tabs).toHaveLength(1);
      expect(state).toHaveCurrentTab({ id: tab1.id });
    });

    test("removes slot when all tabs in slot are invalidated", () => {
      const tab1 = createSessionTab();
      const tab2 = createSessionTab();

      useTabs.getState().openNew(tab1);
      useTabs.getState().openCurrent(tab2);

      expect(useTabs.getState().history.size).toBe(1);
      expect(useTabs.getState()).toHaveHistoryLength(2);

      useTabs.getState().invalidateResource("sessions", tab1.id);
      useTabs.getState().invalidateResource("sessions", tab2.id);

      const state = useTabs.getState();
      expect(state.tabs).toHaveLength(0);
      expect(state.currentTab).toBeNull();
      expect(state.history.size).toBe(0);
    });

    test("updates canGoBack and canGoNext after invalidation", () => {
      const tab1 = createSessionTab();
      const tab2 = createSessionTab();
      const tab3 = createSessionTab();

      useTabs.getState().openNew(tab1);
      useTabs.getState().openCurrent(tab2);
      useTabs.getState().openCurrent(tab3);
      useTabs.getState().goBack();

      expect(useTabs.getState()).toHaveCurrentTab({ id: tab2.id });
      expect(useTabs.getState().canGoBack).toBe(true);
      expect(useTabs.getState().canGoNext).toBe(true);

      useTabs.getState().invalidateResource("sessions", tab1.id);

      const state = useTabs.getState();
      expect(state).toHaveCurrentTab({ id: tab2.id });
      expect(state.canGoBack).toBe(false);
      expect(state.canGoNext).toBe(true);
    });

    test("does not affect unrelated tabs", () => {
      const tab1 = createSessionTab();
      const tab2 = createSessionTab();
      const tab3 = createSessionTab();

      useTabs.getState().openNew(tab1);
      useTabs.getState().openNew(tab2);
      useTabs.getState().openNew(tab3);

      expect(useTabs.getState().tabs).toHaveLength(3);

      useTabs.getState().invalidateResource("sessions", tab2.id);

      const state = useTabs.getState();
      expect(state.tabs).toHaveLength(2);
      expect(
        state.tabs.some((t) => t.type === "sessions" && t.id === tab1.id),
      ).toBe(true);
      expect(
        state.tabs.some((t) => t.type === "sessions" && t.id === tab2.id),
      ).toBe(false);
      expect(
        state.tabs.some((t) => t.type === "sessions" && t.id === tab3.id),
      ).toBe(true);
    });

    test("handles invalidation of non-existent resource gracefully", () => {
      const tab1 = createSessionTab();

      useTabs.getState().openNew(tab1);

      expect(useTabs.getState().tabs).toHaveLength(1);

      useTabs.getState().invalidateResource("sessions", "non-existent-id");

      const state = useTabs.getState();
      expect(state.tabs).toHaveLength(1);
      expect(state).toHaveCurrentTab({ id: tab1.id });
    });
  });
});
