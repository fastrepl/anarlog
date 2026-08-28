import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { createRef, type ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  MultiSelectionBar,
  SelectionMenu,
  selectionMenuStyles,
} from "./selection-menu";
import type { TranscriptContextMenuRequest } from "./selection-menu";

import { setSessionFabSelectionHost } from "~/session/components/floating/selection-slot";
import { expectNotStyle, expectStyle } from "~/session/stylex-test";

vi.mock("@floating-ui/react", () => ({
  autoUpdate: vi.fn(),
  flip: vi.fn(),
  FloatingPortal: ({ children }: { children: ReactNode }) => children,
  offset: vi.fn(),
  shift: vi.fn(),
  useFloating: () => ({
    refs: {
      setFloating: vi.fn(),
      setPositionReference: vi.fn(),
    },
    floatingStyles: {},
    update: vi.fn(),
  }),
}));

vi.mock("./speaker-assign", () => ({
  SpeakerParticipantPicker: ({
    onSelect,
  }: {
    onSelect?: (humanId: string) => void;
  }) => (
    <button type="button" onClick={() => onSelect?.("human-1")}>
      Confirm
    </button>
  ),
}));

vi.mock("~/shared/hooks/useAutoCloser", () => ({
  useAutoCloser: () => ({ current: null }),
}));

afterEach(() => {
  cleanup();
  setSessionFabSelectionHost(null);
});

describe("SelectionMenu", () => {
  it("keeps the speaker picker inside the viewport without a back row", () => {
    const request = {
      id: "request-1",
      range: {
        getBoundingClientRect: () => new DOMRect(20, 700, 100, 20),
        getClientRects: () => [],
        startOffset: 0,
        endOffset: 4,
      },
      selection: {
        sessionId: "session-1",
        text: "Test",
        startMs: 0,
        groups: [],
      },
      x: 20,
      y: 700,
    } as unknown as TranscriptContextMenuRequest;

    render(
      <SelectionMenu
        containerRef={createRef()}
        contextRequest={request}
        audioExists={false}
        onContextClose={vi.fn()}
        onAssignSpeaker={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Change speaker" }));

    expect(screen.queryByRole("button", { name: "Back" })).toBeNull();
    const confirm = screen.getByRole("button", { name: "Confirm" });
    expectStyle(confirm.parentElement, selectionMenuStyles.menu);
    expectStyle(confirm.parentElement, selectionMenuStyles.speakerMenu);
  });

  it("hides playback when the transcript has no audio", () => {
    render(
      <SelectionMenu
        containerRef={createRef()}
        contextRequest={createContextRequest()}
        audioExists={false}
        onContextClose={vi.fn()}
        onAssignSpeaker={vi.fn()}
      />,
    );

    expect(screen.queryByRole("button", { name: "Play from here" })).toBeNull();
    expect(screen.getByRole("button", { name: "Change speaker" })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Copy$/ })).toBeTruthy();
  });

  it("keeps playback available when the transcript has audio", () => {
    render(
      <SelectionMenu
        containerRef={createRef()}
        contextRequest={createContextRequest()}
        audioExists
        onContextClose={vi.fn()}
        onAssignSpeaker={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "Play from here" })).toBeTruthy();
  });
});

describe("MultiSelectionBar", () => {
  const selection = {
    sessionId: "session-1",
    text: "Hello",
    startMs: 0,
    groups: [],
  };

  it("keeps merge disabled until contiguous entries are selected", () => {
    render(
      <MultiSelectionBar
        selection={selection}
        entryCount={2}
        canMerge={false}
        onClear={vi.fn()}
        onAssignSpeaker={vi.fn()}
        onMerge={vi.fn()}
      />,
    );

    expect(
      (screen.getByRole("button", { name: "Merge" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });

  it("assigns the current selection and then clears it", async () => {
    const onAssignSpeaker = vi.fn(() => Promise.resolve());
    const onClear = vi.fn();

    render(
      <MultiSelectionBar
        selection={selection}
        entryCount={2}
        onClear={onClear}
        onAssignSpeaker={onAssignSpeaker}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Change speaker" }));
    fireEvent.click(screen.getByRole("button", { name: "Confirm" }));

    await waitFor(() => {
      expect(onAssignSpeaker).toHaveBeenCalledWith(selection, "human-1");
      expect(onClear).toHaveBeenCalled();
    });
  });

  it("merges contiguous entries and then clears the selection", async () => {
    const onMerge = vi.fn(() => Promise.resolve());
    const onClear = vi.fn();

    render(
      <MultiSelectionBar
        selection={selection}
        entryCount={2}
        canMerge
        onClear={onClear}
        onAssignSpeaker={vi.fn()}
        onMerge={onMerge}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Merge" }));

    await waitFor(() => {
      expect(onMerge).toHaveBeenCalled();
      expect(onClear).toHaveBeenCalled();
    });
  });

  it("renders into the session FAB selection slot when it is present", () => {
    const host = document.createElement("div");
    document.body.append(host);
    setSessionFabSelectionHost(host);

    render(
      <MultiSelectionBar
        selection={selection}
        entryCount={2}
        onClear={vi.fn()}
        onAssignSpeaker={vi.fn()}
      />,
    );

    expect(host.textContent).toContain("2 selected");
    expectStyle(host.firstElementChild, selectionMenuStyles.selectionBar);
    expectNotStyle(host.firstElementChild, selectionMenuStyles.floating);

    host.remove();
  });
});

function createContextRequest() {
  return {
    id: crypto.randomUUID(),
    range: {
      getBoundingClientRect: () => new DOMRect(20, 700, 100, 20),
      getClientRects: () => [],
      startOffset: 0,
      endOffset: 4,
    },
    selection: {
      sessionId: "session-1",
      text: "Test",
      startMs: 0,
      groups: [],
    },
    x: 20,
    y: 700,
  } as unknown as TranscriptContextMenuRequest;
}
