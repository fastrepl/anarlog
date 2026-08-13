import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { createRef, type ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { SelectionMenu } from "./selection-menu";
import type { TranscriptContextMenuRequest } from "./selection-menu";

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
  SpeakerParticipantPicker: () => <button type="button">Confirm</button>,
}));

vi.mock("~/shared/hooks/useAutoCloser", () => ({
  useAutoCloser: () => ({ current: null }),
}));

afterEach(cleanup);

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
    expect(confirm.parentElement?.className).toContain(
      "max-h-[min(28rem,calc(100vh-1rem))]",
    );
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
