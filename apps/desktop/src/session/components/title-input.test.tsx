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

import { TitleInput } from "./title-input";

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

  it.each([false, true])(
    "shows the persisted title when it loads after mount (focused=%s)",
    (focused) => {
      hoisted.storeTitle = undefined;

      const { rerender } = renderTitleInput();

      const input = screen.getByPlaceholderText("Untitled");
      if (focused) fireEvent.focus(input);
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
    },
  );

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
});
