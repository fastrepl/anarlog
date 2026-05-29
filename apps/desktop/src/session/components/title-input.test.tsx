import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { TitleInput } from "./title-input";

const hoisted = vi.hoisted(() => ({
  clearLiveTitle: vi.fn(),
  runEscapeShortcut: vi.fn(),
  setLiveTitle: vi.fn(),
  store: {
    addCellListener: vi.fn(() => "listener-id"),
    delListener: vi.fn(),
    getCell: vi.fn(() => "Untitled"),
  },
}));

vi.mock("usehooks-ts", () => ({
  useResizeObserver: vi.fn(),
}));

vi.mock("~/ai/hooks", () => ({
  useTitleGenerating: () => false,
}));

vi.mock("~/shared/useTabsShortcuts", () => ({
  useMainEscapeShortcutAction: () => hoisted.runEscapeShortcut,
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  UI: {
    useSetPartialRowCallback: () => vi.fn(),
    useStore: () => hoisted.store,
  },
}));

vi.mock("~/store/zustand/live-title", () => ({
  useLiveTitle: (
    selector: (state: {
      clearTitle: typeof hoisted.clearLiveTitle;
      setTitle: typeof hoisted.setLiveTitle;
    }) => unknown,
  ) =>
    selector({
      clearTitle: hoisted.clearLiveTitle,
      setTitle: hoisted.setLiveTitle,
    }),
}));

describe("TitleInput", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("runs the main escape shortcut directly from the title field", () => {
    render(
      <TitleInput
        tab={{
          active: true,
          id: "session-1",
          pinned: false,
          slotId: "slot-1",
          state: { autoStart: null, view: null },
          type: "sessions",
        }}
      />,
    );

    fireEvent.keyDown(screen.getByPlaceholderText("Untitled"), {
      key: "Escape",
    });

    expect(hoisted.runEscapeShortcut).toHaveBeenCalledTimes(1);
  });
});
