import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const hoisted = vi.hoisted(() => ({
  clearSelection: vi.fn(),
  close: vi.fn(),
  handlers: new Map<string, () => void>(),
  newNote: vi.fn(),
  newNoteAndListen: vi.fn(),
  openNew: vi.fn(),
  restoreLastClosedTab: vi.fn(),
  select: vi.fn(),
  selectNext: vi.fn(),
  selectPrev: vi.fn(),
  setPendingCloseConfirmationTab: vi.fn(),
  startNewChat: vi.fn(),
  unpin: vi.fn(),
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
      startNewChat: hoisted.startNewChat,
    },
  }),
}));

vi.mock("~/shared/useNewNote", () => ({
  useNewNote: ({ behavior }: { behavior?: "new" | "current" } = {}) =>
    behavior === "current" ? vi.fn() : hoisted.newNote,
  useNewNoteAndListen: () => hoisted.newNoteAndListen,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (
    selector: (state: {
      clearSelection: typeof hoisted.clearSelection;
      close: typeof hoisted.close;
      currentTab: null;
      openNew: typeof hoisted.openNew;
      restoreLastClosedTab: typeof hoisted.restoreLastClosedTab;
      select: typeof hoisted.select;
      selectNext: typeof hoisted.selectNext;
      selectPrev: typeof hoisted.selectPrev;
      setPendingCloseConfirmationTab: typeof hoisted.setPendingCloseConfirmationTab;
      tabs: [];
      unpin: typeof hoisted.unpin;
    }) => unknown,
  ) =>
    selector({
      tabs: [],
      currentTab: null,
      clearSelection: hoisted.clearSelection,
      close: hoisted.close,
      select: hoisted.select,
      selectNext: hoisted.selectNext,
      selectPrev: hoisted.selectPrev,
      restoreLastClosedTab: hoisted.restoreLastClosedTab,
      openNew: hoisted.openNew,
      unpin: hoisted.unpin,
      setPendingCloseConfirmationTab: hoisted.setPendingCloseConfirmationTab,
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

import { useMain2TabsShortcuts } from "~/main2/useTabsShortcuts";

describe("useMain2TabsShortcuts", () => {
  beforeEach(() => {
    hoisted.handlers.clear();
    hoisted.clearSelection.mockClear();
    hoisted.openNew.mockClear();
  });

  it("binds mod+t to home instead of opening a classic empty tab", () => {
    renderHook(() => useMain2TabsShortcuts());

    const handler = hoisted.handlers.get("mod+t");
    expect(handler).toBeTruthy();

    handler?.();

    expect(hoisted.clearSelection).toHaveBeenCalledTimes(1);
    expect(hoisted.openNew).not.toHaveBeenCalled();
  });
});
