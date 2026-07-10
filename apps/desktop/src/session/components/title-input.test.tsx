import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { type ComponentProps } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@hypr/ui/components/ui/tooltip";

import { TitleInput } from "./title-input";

const hoisted = vi.hoisted(() => ({
  clearLiveTitle: vi.fn(),
  setStoreTitle: vi.fn((_title?: string) => Promise.resolve()),
  setLiveTitle: vi.fn(),
  storeTitle: "Untitled" as string | undefined,
}));

vi.mock("usehooks-ts", () => ({
  useResizeObserver: vi.fn(),
}));

vi.mock("~/ai/hooks", () => ({
  useTitleGenerating: () => false,
}));

vi.mock("~/session/queries", () => ({
  useSession: () => ({ title: hoisted.storeTitle }),
  useUpdateSession: () => (changes: { title?: string }) =>
    hoisted.setStoreTitle(changes.title),
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
    hoisted.storeTitle = "Untitled";
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

  it("does not handle IME confirmation keys as title navigation", () => {
    const onTransferContentToEditor = vi.fn();
    const onFocusEditorAtStart = vi.fn();
    renderTitleInput({
      onFocusEditorAtStart,
      onTransferContentToEditor,
    });

    const input = screen.getByPlaceholderText("Untitled");
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: "안" } });
    fireEvent.keyDown(input, {
      key: "Enter",
      keyCode: 229,
    });

    expect(hoisted.setStoreTitle).not.toHaveBeenCalled();
    expect(hoisted.clearLiveTitle).not.toHaveBeenCalled();
    expect(onTransferContentToEditor).not.toHaveBeenCalled();
    expect(onFocusEditorAtStart).not.toHaveBeenCalled();
  });

  it("left-aligns the empty title field without a generate button", () => {
    hoisted.storeTitle = "";

    renderTitleInput();

    const input = screen.getByPlaceholderText("Untitled");
    expect(input.parentElement?.className).toContain("relative");
    expect(input.parentElement?.className).toContain("max-w-full");
    expect(input.parentElement?.className).toContain("text-xl");
    expect(input.parentElement?.className).toContain("font-semibold");
    expect(input.parentElement?.classList.contains("w-full")).toBe(false);
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

  it("uses sans-serif styling for breadcrumb titles", () => {
    renderTitleInput({ variant: "breadcrumb" });

    const input = screen.getByPlaceholderText("Untitled");

    expect(input.parentElement?.className).toContain("text-sm");
    expect(input.parentElement?.className).toContain("leading-5");
    expect(input.parentElement?.className).not.toContain("font-mono");
    expect(input.className).toContain("text-sm");
    expect(input.className).toContain("leading-5");
    expect(input.className).toContain("appearance-none");
    expect(input.className).toContain("p-0");
    expect(input.className).toContain("truncate");
    expect(input.className).not.toContain("font-mono");
  });

  it("keeps focused breadcrumb titles horizontally scrollable", () => {
    renderTitleInput({ variant: "breadcrumb" });

    const input = screen.getByPlaceholderText("Untitled");
    fireEvent.focus(input);

    expect(input.className).not.toContain("truncate");
    expect(input.className).toContain("overflow-x-auto");
    expect(input.className).toContain("whitespace-nowrap");
  });

  it("uses the flexible title layout for whitespace-only titles", () => {
    hoisted.storeTitle = "          ";

    renderTitleInput();

    const input = screen.getByPlaceholderText("Untitled");
    expect(input.className).toContain("w-full");
    expect(input.parentElement?.style.width).toBe("calc(10ch + 2px)");
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

  it("updates when the persisted title loads after mount", () => {
    hoisted.storeTitle = undefined;

    const { rerender } = renderTitleInput();

    const input = screen.getByPlaceholderText("Untitled");
    expect((input as HTMLInputElement).value).toBe("");

    hoisted.storeTitle = "Spotify Leadership Transition";
    rerender(
      <TooltipProvider>
        <TitleInput
          tab={{
            active: true,
            id: "session-1",
            pinned: false,
            slotId: "slot-1",
            state: { autoStart: null, view: { type: "raw" } },
            type: "sessions",
          }}
        />
      </TooltipProvider>,
    );

    expect(
      (screen.getByPlaceholderText("Untitled") as HTMLInputElement).value,
    ).toBe("Spotify Leadership Transition");
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
