import { renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { Tab } from "~/store/zustand/tabs";

const mocks = vi.hoisted(() => ({
  currentTab: null as Tab | null,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: () => ({ currentTab: mocks.currentTab }),
}));

import { useSessionTab } from "./use-session-tab";

describe("useSessionTab", () => {
  it("clears the retained session when the empty tab becomes active", () => {
    mocks.currentTab = {
      active: true,
      id: "session-1",
      pinned: false,
      slotId: "slot-1",
      state: { autoStart: null, view: null },
      type: "sessions",
    };
    const { result, rerender } = renderHook(() => useSessionTab());

    expect(result.current.currentSessionId).toBe("session-1");

    mocks.currentTab = {
      active: true,
      pinned: false,
      requestId: "edit-1",
      slotId: "slot-1",
      type: "edit",
    };
    rerender();

    expect(result.current.currentSessionId).toBe("session-1");

    mocks.currentTab = {
      active: true,
      pinned: false,
      slotId: "slot-1",
      type: "empty",
    };
    rerender();

    expect(result.current.currentSessionId).toBeUndefined();
    expect(result.current.getSessionId()).toBeUndefined();
  });
});
