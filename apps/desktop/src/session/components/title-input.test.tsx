import * as stylex from "@stylexjs/stylex";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { type ComponentProps } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@anlg/ui/components/ui/tooltip";

import { TitleInput, titleInputStyles } from "./title-input";

import { expectNotStyle, expectStyle } from "~/session/stylex-test";

const hoisted = vi.hoisted(() => ({
  clearLiveTitle: vi.fn(),
  markLiveTitlePersisted: vi.fn(),
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
      markTitlePersisted: typeof hoisted.markLiveTitlePersisted;
      setTitle: typeof hoisted.setLiveTitle;
    }) => unknown,
  ) =>
    selector({
      clearTitle: hoisted.clearLiveTitle,
      markTitlePersisted: hoisted.markLiveTitlePersisted,
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

  it("keeps the live title until the persisted title settles", async () => {
    let resolveUpdate: (() => void) | undefined;
    hoisted.setStoreTitle.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        resolveUpdate = resolve;
      }),
    );
    renderTitleInput();

    const input = screen.getByPlaceholderText("Untitled");
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: "Customer call" } });
    fireEvent.blur(input);

    expect(hoisted.setStoreTitle).toHaveBeenCalledWith("Customer call");
    expect(hoisted.clearLiveTitle).not.toHaveBeenCalled();

    resolveUpdate?.();

    await waitFor(() => {
      expect(hoisted.markLiveTitlePersisted).toHaveBeenCalledWith(
        "session-1",
        "Customer call",
        "Untitled",
      );
    });
    expect(hoisted.clearLiveTitle).not.toHaveBeenCalled();
  });

  it("keeps the live title when Enter persists the title", async () => {
    let resolveUpdate: (() => void) | undefined;
    hoisted.setStoreTitle.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        resolveUpdate = resolve;
      }),
    );
    renderTitleInput();

    const input = screen.getByPlaceholderText("Untitled");
    input.focus();
    fireEvent.change(input, { target: { value: "Customer call" } });
    (input as HTMLInputElement).setSelectionRange(3, 3);
    fireEvent.keyDown(input, { key: "Enter" });

    expect(hoisted.setStoreTitle).toHaveBeenCalledWith("Customer call");
    expect(hoisted.setLiveTitle).toHaveBeenLastCalledWith(
      "session-1",
      "Customer call",
    );
    expect(hoisted.clearLiveTitle).not.toHaveBeenCalled();

    resolveUpdate?.();

    await waitFor(() => {
      expect(hoisted.markLiveTitlePersisted).toHaveBeenCalledWith(
        "session-1",
        "Customer call",
        "Untitled",
      );
    });
    expect(hoisted.clearLiveTitle).not.toHaveBeenCalled();
  });

  it("does not let an earlier blur clear a later Enter title", async () => {
    let resolveBlurUpdate: (() => void) | undefined;
    let resolveEnterUpdate: (() => void) | undefined;
    hoisted.setStoreTitle
      .mockReturnValueOnce(
        new Promise<void>((resolve) => {
          resolveBlurUpdate = resolve;
        }),
      )
      .mockReturnValueOnce(
        new Promise<void>((resolve) => {
          resolveEnterUpdate = resolve;
        }),
      );
    renderTitleInput();

    const input = screen.getByPlaceholderText("Untitled");
    input.focus();
    fireEvent.change(input, { target: { value: "Customer call" } });
    input.blur();
    input.focus();
    fireEvent.keyDown(input, { key: "Enter" });

    await act(async () => {
      resolveBlurUpdate?.();
      await Promise.resolve();
    });

    expect(hoisted.clearLiveTitle).not.toHaveBeenCalled();
    expect(hoisted.markLiveTitlePersisted).not.toHaveBeenCalled();

    resolveEnterUpdate?.();

    await waitFor(() => {
      expect(hoisted.markLiveTitlePersisted).toHaveBeenCalledWith(
        "session-1",
        "Customer call",
        "Untitled",
      );
    });
    expect(hoisted.clearLiveTitle).not.toHaveBeenCalled();
  });

  it("does not clear a newer live title when an earlier update settles", async () => {
    let resolveUpdate: (() => void) | undefined;
    hoisted.setStoreTitle.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        resolveUpdate = resolve;
      }),
    );
    renderTitleInput();

    const input = screen.getByPlaceholderText("Untitled");
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: "Customer call" } });
    fireEvent.blur(input);
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: "Customer follow-up" } });

    await act(async () => {
      resolveUpdate?.();
      await Promise.resolve();
    });

    expect(hoisted.clearLiveTitle).not.toHaveBeenCalled();
    expect(hoisted.markLiveTitlePersisted).not.toHaveBeenCalled();
  });

  it("left-aligns the empty title field without a generate button", () => {
    hoisted.storeTitle = "";

    renderTitleInput();

    const input = screen.getByPlaceholderText("Untitled");
    expectStyle(input.parentElement, titleInputStyles.shell);
    expectStyle(input.parentElement, titleInputStyles.titleText);
    expectStyle(input, titleInputStyles.input);
    expectStyle(input, titleInputStyles.titleText);
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

    expectStyle(input.parentElement, titleInputStyles.breadcrumbText);
    expectStyle(input, titleInputStyles.breadcrumbInput);
    expectStyle(input, titleInputStyles.breadcrumbText);
    expectStyle(input, titleInputStyles.breadcrumbInputTruncated);
  });

  it("keeps focused breadcrumb titles horizontally scrollable", () => {
    renderTitleInput({ variant: "breadcrumb" });

    const input = screen.getByPlaceholderText("Untitled");
    fireEvent.focus(input);

    expectStyle(input, titleInputStyles.breadcrumbInputFocused);
    expectNotStyle(input, titleInputStyles.breadcrumbInputTruncated);
  });

  it("uses the flexible title layout for whitespace-only titles", () => {
    hoisted.storeTitle = "          ";

    renderTitleInput();

    const input = screen.getByPlaceholderText("Untitled");
    expectStyle(input, titleInputStyles.input);
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
    expectStyle(input, titleInputStyles.concealedInput);
    expect(input.parentElement?.style.maskImage).toBe(
      "linear-gradient(to right, black 0, black calc(100% - 28px), transparent 100%)",
    );
    expectStyle(input.parentElement, stylex.defaultMarker());
    expectStyle(overlay, titleInputStyles.hoverOverlay);
    expectStyle(hoverTitle, titleInputStyles.hoverTitle);
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

  it("shows the persisted title when it loads while the field is focused", () => {
    hoisted.storeTitle = undefined;

    const { rerender } = renderTitleInput();

    const input = screen.getByPlaceholderText("Untitled");
    fireEvent.focus(input);
    expect((input as HTMLInputElement).value).toBe("");

    hoisted.storeTitle = "founders sync";
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
    ).toBe("founders sync");
  });

  it("does not persist an unedited title on blur", () => {
    hoisted.storeTitle = "founders sync";

    renderTitleInput();

    const input = screen.getByPlaceholderText("Untitled");
    fireEvent.focus(input);
    fireEvent.blur(input);

    expect(hoisted.setStoreTitle).not.toHaveBeenCalled();
  });

  it("does not keep an empty draft when switching notes", () => {
    hoisted.storeTitle = "";

    const { rerender } = renderTitleInput();

    const input = screen.getByPlaceholderText("Untitled");
    fireEvent.focus(input);
    expect((input as HTMLInputElement).value).toBe("");

    hoisted.storeTitle = "founders sync";
    rerender(
      <TooltipProvider>
        <TitleInput
          tab={{
            active: true,
            id: "session-2",
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
    ).toBe("founders sync");
    expect(hoisted.setStoreTitle).not.toHaveBeenCalled();
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
