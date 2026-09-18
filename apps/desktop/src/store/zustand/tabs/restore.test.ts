import "./test-matchers";

import { beforeEach, describe, expect, test } from "vitest";

import { useTabs } from ".";
import { createSessionTab, resetTabsStore, seedTabsStore } from "./test-utils";

const openTabs = (...tabs: ReturnType<typeof createSessionTab>[]) => {
  tabs.forEach((tab) => useTabs.getState().openNew(tab));
};

describe("Restore", () => {
  beforeEach(() => {
    resetTabsStore();
  });

  describe("closedTabs tracking", () => {
    test("adds closed tab to closedTabs", () => {
      const tab1 = createSessionTab({ id: "session-1", active: true });
      const tab2 = createSessionTab({ id: "session-2" });
      openTabs(tab1, tab2);

      useTabs.getState().close(tab1);

      expect(useTabs.getState().closedTabs).toMatchObject([
        { id: "session-1" },
      ]);
    });

    test("preserves close order", () => {
      const tab1 = createSessionTab({ id: "session-1", active: true });
      const tab2 = createSessionTab({ id: "session-2" });
      const tab3 = createSessionTab({ id: "session-3" });
      openTabs(tab1, tab2, tab3);

      useTabs.getState().close(tab1);
      useTabs.getState().close(tab2);

      expect(useTabs.getState().closedTabs).toMatchObject([
        { id: "session-1" },
        { id: "session-2" },
      ]);
    });

    test("caps at 10 entries", () => {
      const tabs = Array.from({ length: 12 }, (_, i) =>
        createSessionTab({ id: `session-${i}`, active: i === 0 }),
      );
      openTabs(...tabs);

      tabs.slice(0, 11).forEach((tab) => useTabs.getState().close(tab));

      const { closedTabs } = useTabs.getState();
      expect(closedTabs).toHaveLength(10);
      expect(closedTabs[0]).toMatchObject({ id: "session-1" });
      expect(closedTabs[9]).toMatchObject({ id: "session-10" });
    });
  });

  describe("restoreLastClosedTab", () => {
    test("no-op when closedTabs is empty", () => {
      const tab = createSessionTab({ id: "session-1", active: true });
      openTabs(tab);

      useTabs.getState().restoreLastClosedTab();

      expect(useTabs.getState().tabs).toHaveLength(1);
    });

    test("restores most recently closed tab (LIFO)", () => {
      const tab1 = createSessionTab({ id: "session-1", active: true });
      const tab2 = createSessionTab({ id: "session-2" });
      const tab3 = createSessionTab({ id: "session-3" });
      openTabs(tab1, tab2, tab3);
      useTabs.getState().close(tab1);
      useTabs.getState().close(tab2);

      useTabs.getState().restoreLastClosedTab();
      expect(useTabs.getState().closedTabs).toMatchObject([
        { id: "session-1" },
      ]);

      useTabs.getState().restoreLastClosedTab();
      expect(useTabs.getState().closedTabs).toHaveLength(0);
    });

    test("assigns new slotId to restored tab", () => {
      const tab1 = createSessionTab({ id: "session-1", active: true });
      const tab2 = createSessionTab({ id: "session-2" });
      openTabs(tab1, tab2);
      const originalSlotId = useTabs.getState().tabs[0].slotId;

      useTabs.getState().close(tab1);
      useTabs.getState().restoreLastClosedTab();

      const restoredTab = useTabs
        .getState()
        .tabs.find((t) => t.type === "sessions" && t.id === "session-1");
      expect(restoredTab!.slotId).not.toBe(originalSlotId);
    });
  });

  test("seedTabsStore preserves closedTabs", () => {
    const closedTab = createSessionTab({ id: "closed-session" });
    seedTabsStore({ closedTabs: [closedTab] });

    expect(useTabs.getState().closedTabs).toMatchObject([
      { id: "closed-session" },
    ]);
  });
});
