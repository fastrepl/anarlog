import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const hoisted = vi.hoisted(() => ({
  handlers: new Map<string, () => void>(),
  openNew: vi.fn(),
}));

vi.mock("react-hotkeys-hook", () => ({
  useHotkeys: (keys: string, handler: () => void) => {
    hoisted.handlers.set(keys, handler);
  },
}));

vi.mock("~/auth/billing", () => ({
  useBillingAccess: () => ({ isPro: true }),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    chat: {
      mode: "FloatingClosed",
      sendEvent: vi.fn(),
      startNewChat: vi.fn(),
    },
  }),
}));

vi.mock("~/shared/useNewNote", () => ({
  useNewNote: () => vi.fn(),
  useNewNoteAndListen: () => vi.fn(),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (
    selector: (state: {
      clearSelection: () => void;
      close: () => void;
      currentTab: null;
      openNew: typeof hoisted.openNew;
      restoreLastClosedTab: () => void;
      select: () => void;
      selectNext: () => void;
      selectPrev: () => void;
      setPendingCloseConfirmationTab: () => void;
      tabs: [];
      unpin: () => void;
    }) => unknown,
  ) =>
    selector({
      tabs: [],
      currentTab: null,
      clearSelection: vi.fn(),
      close: vi.fn(),
      select: vi.fn(),
      selectNext: vi.fn(),
      selectPrev: vi.fn(),
      restoreLastClosedTab: vi.fn(),
      openNew: hoisted.openNew,
      unpin: vi.fn(),
      setPendingCloseConfirmationTab: vi.fn(),
    }),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (
    selector: (state: {
      live: { sessionId: string | null; status: string };
    }) => unknown,
  ) =>
    selector({
      live: { sessionId: null, status: "idle" },
    }),
}));

import { useClassicMainTabsShortcuts } from "~/main/useTabsShortcuts";

describe("useClassicMainTabsShortcuts", () => {
  beforeEach(() => {
    hoisted.handlers.clear();
    hoisted.openNew.mockClear();
  });

  it("binds mod+t to open a classic empty tab", () => {
    renderHook(() => useClassicMainTabsShortcuts());

    const handler = hoisted.handlers.get("mod+t");
    expect(handler).toBeTruthy();

    handler?.();

    expect(hoisted.openNew).toHaveBeenCalledWith({ type: "empty" });
  });
});
