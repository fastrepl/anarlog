import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { type ComponentProps } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@hypr/ui/components/ui/tooltip";

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

const renderTitleInput = (
  props: Partial<ComponentProps<typeof TitleInput>> = {},
) =>
  render(
    <TooltipProvider>
      <TitleInput
        tab={{
          active: true,
          id: "session-1",
          pinned: false,
          slotId: "slot-1",
          state: { autoStart: null, view: null },
          type: "sessions",
        }}
        {...props}
      />
    </TooltipProvider>,
  );

describe("TitleInput", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    hoisted.store.getCell.mockImplementation(() => "Untitled");
  });

  afterEach(() => {
    cleanup();
  });

  it("runs the main escape shortcut directly from the title field", () => {
    renderTitleInput();

    fireEvent.keyDown(screen.getByPlaceholderText("Untitled"), {
      key: "Escape",
    });

    expect(hoisted.runEscapeShortcut).toHaveBeenCalledTimes(1);
  });

  it("keeps the empty title layout at placeholder width", () => {
    hoisted.store.getCell.mockReturnValueOnce("");

    renderTitleInput({ onGenerateTitle: vi.fn() });

    const input = screen.getByPlaceholderText("Untitled");
    expect(input.parentElement?.className).toContain("min-w-fit");
    expect(input.parentElement?.className).toContain("flex-none");
    expect(
      screen.getByRole("button", { name: "Regenerate title" }),
    ).toBeTruthy();
  });

  it("uses the flexible title layout for whitespace-only titles", () => {
    hoisted.store.getCell.mockReturnValueOnce("          ");

    renderTitleInput({ onGenerateTitle: vi.fn() });

    const input = screen.getByPlaceholderText("Untitled");
    expect(input.parentElement?.className).toContain("min-w-0");
    expect(input.parentElement?.className).toContain("flex-1");
    expect(
      screen.queryByRole("button", { name: "Regenerate title" }),
    ).toBeNull();
  });
});
