import { beforeEach, describe, expect, test, vi } from "vitest";

import { type Tab, useTabs } from ".";
import { createSessionTab, resetTabsStore, seedTabsStore } from "./test-utils";

describe("Tab Lifecycle", () => {
  beforeEach(() => {
    seedTabsStore();
  });

  test("registerOnClose triggers handler when close removes tab", () => {
    const tab = createSessionTab({ active: true });

    const handler = vi.fn();
    useTabs.getState().openCurrent(tab);
    useTabs.getState().registerOnClose(handler);
    useTabs.getState().close(tab);

    expect(handler).toHaveBeenCalledTimes(1);
    expect(handler).toHaveBeenCalledWith(
      expect.objectContaining({ id: tab.id, type: "sessions" }),
    );
  });

  test("registerOnClose triggers handler when openCurrent replaces tab", () => {
    const tab1 = createSessionTab({ active: true });
    const tab2 = createSessionTab({ active: true });

    const handler = vi.fn();
    useTabs.getState().openCurrent(tab1);
    useTabs.getState().registerOnClose(handler);
    useTabs.getState().openCurrent(tab2);

    expect(handler).toHaveBeenCalledTimes(1);
    expect(handler).toHaveBeenCalledWith(
      expect.objectContaining({ id: tab1.id, type: "sessions" }),
    );
  });

  test("registerOnClose handler receives correct tab when multiple tabs close", () => {
    const tab1 = createSessionTab({ active: true });
    const tab2 = createSessionTab({ active: true });

    const closedTabs: Tab[] = [];
    const handler = vi.fn((tab: Tab) => closedTabs.push(tab));

    useTabs.getState().registerOnClose(handler);
    useTabs.getState().openCurrent(tab1);
    useTabs.getState().openNew(tab2);
    useTabs.getState().close(tab2);

    expect(closedTabs).toHaveLength(1);
    expect(closedTabs[0]).toMatchObject({ id: tab2.id, type: "sessions" });
  });

  test("registerOnEmpty fires when tabs become empty", () => {
    const tab = createSessionTab({ active: true });
    const onEmpty = vi.fn();

    resetTabsStore();
    useTabs.getState().registerOnEmpty(onEmpty);
    useTabs.getState().openCurrent(tab);
    useTabs.getState().close(tab);

    expect(onEmpty).toHaveBeenCalledTimes(1);
  });

  test("registerOnClose is idempotent", () => {
    const tab = createSessionTab({ active: true });
    const handler = vi.fn();

    useTabs.getState().registerOnClose(handler);
    useTabs.getState().registerOnClose(handler);
    useTabs.getState().registerOnClose(handler);

    useTabs.getState().openCurrent(tab);
    useTabs.getState().close(tab);

    expect(handler).toHaveBeenCalledTimes(1);
  });

  test("registerOnEmpty is idempotent", () => {
    const tab = createSessionTab({ active: true });
    const handler = vi.fn();

    resetTabsStore();
    useTabs.getState().registerOnEmpty(handler);
    useTabs.getState().registerOnEmpty(handler);
    useTabs.getState().registerOnEmpty(handler);

    useTabs.getState().openCurrent(tab);
    useTabs.getState().close(tab);

    expect(handler).toHaveBeenCalledTimes(1);
  });
});
