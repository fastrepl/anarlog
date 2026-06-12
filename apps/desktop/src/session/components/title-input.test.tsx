import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { type ComponentProps } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@hypr/ui/components/ui/tooltip";

import { TitleInput } from "./title-input";

const hoisted = vi.hoisted(() => ({
  clearLiveTitle: vi.fn(),
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

  it("does not route escape from the title field into tab navigation", () => {
    renderTitleInput();

    fireEvent.keyDown(screen.getByPlaceholderText("Untitled"), {
      key: "Escape",
    });

    expect(hoisted.clearLiveTitle).not.toHaveBeenCalled();
  });

  it("left-aligns the empty title field without a generate button", () => {
    hoisted.store.getCell.mockReturnValueOnce("");

    renderTitleInput();

    const input = screen.getByPlaceholderText("Untitled");
    expect(input.parentElement?.className).toContain("relative");
    expect(input.className).toContain("text-left");
    expect(
      screen.queryByRole("button", { name: "Regenerate title" }),
    ).toBeNull();
  });

  it("keeps the title field out of the header drag region", () => {
    renderTitleInput();

    const input = screen.getByPlaceholderText("Untitled");

    expect(input.getAttribute("data-tauri-drag-region")).toBe("false");
    expect(input.parentElement?.getAttribute("data-tauri-drag-region")).toBe(
      "false",
    );
  });

  it("uses the flexible title layout for whitespace-only titles", () => {
    hoisted.store.getCell.mockReturnValueOnce("          ");

    renderTitleInput();

    const input = screen.getByPlaceholderText("Untitled");
    expect(input.className).toContain("w-full");
    expect(
      screen.queryByRole("button", { name: "Regenerate title" }),
    ).toBeNull();
  });

  it("reveals overflowing titles with a hover scroll overlay", () => {
    const title =
      "Product Discovery Pace and Headless Agent Usage Strategy Review";

    renderTitleInput();

    const input = screen.getByPlaceholderText("Untitled");
    Object.defineProperty(input, "clientWidth", {
      configurable: true,
      value: 160,
    });
    Object.defineProperty(input, "scrollWidth", {
      configurable: true,
      value: 420,
    });

    fireEvent.change(input, { target: { value: title } });

    const hoverTitle = screen.getByText(title);
    const overlay = hoverTitle.parentElement;
    expect(input.className).toContain("text-transparent");
    expect(input.parentElement?.style.maskImage).toBe(
      "linear-gradient(to right, black 0, black calc(100% - 28px), transparent 100%)",
    );
    expect(overlay?.className).toContain("justify-start");
    expect(hoverTitle.className).toContain(
      "group-hover/title-input:animate-title-hover-scroll",
    );
    expect(
      hoverTitle.style.getPropertyValue("--title-hover-scroll-distance"),
    ).toBe("-260px");
  });

  it("updates title fades based on horizontal scroll position", () => {
    renderTitleInput();

    const input = screen.getByPlaceholderText("Untitled");
    Object.defineProperty(input, "clientWidth", {
      configurable: true,
      value: 160,
    });
    Object.defineProperty(input, "scrollWidth", {
      configurable: true,
      value: 420,
    });

    fireEvent.change(input, {
      target: {
        value:
          "Product Discovery Pace and Headless Agent Usage Strategy Review",
      },
    });

    const titleInputShell = input.parentElement;
    expect(titleInputShell?.style.maskImage).toBe(
      "linear-gradient(to right, black 0, black calc(100% - 28px), transparent 100%)",
    );

    input.scrollLeft = 130;
    fireEvent.scroll(input);

    expect(titleInputShell?.style.maskImage).toBe(
      "linear-gradient(to right, transparent 0, black 28px, black calc(100% - 28px), transparent 100%)",
    );

    input.scrollLeft = 260;
    fireEvent.scroll(input);

    expect(titleInputShell?.style.maskImage).toBe(
      "linear-gradient(to right, transparent 0, black 28px, black 100%)",
    );
  });
});
