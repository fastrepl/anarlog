import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { FloatingActionButton } from "./index";

import type { Tab } from "~/store/zustand/tabs";
import type { EditorView } from "~/store/zustand/tabs/schema";

const hoisted = vi.hoisted(() => ({
  sendEvent: vi.fn(),
}));

vi.mock("~/shared/chat-cta", () => ({
  ChatCTA: () => (
    <button type="button" onClick={() => hoisted.sendEvent({ type: "OPEN" })}>
      Ask Anarlog anything
    </button>
  ),
}));

describe("FloatingActionButton", () => {
  const tab = {
    type: "sessions",
    id: "session-1",
    active: true,
    pinned: false,
    slotId: "slot-1",
    state: { view: null, autoStart: null },
  } as Extract<Tab, { type: "sessions" }>;

  const renderFloatingActionButton = (
    props: Partial<React.ComponentProps<typeof FloatingActionButton>> = {},
  ) =>
    render(
      <FloatingActionButton
        currentView={{ type: "raw" } as EditorView}
        tab={tab}
        {...props}
      />,
    );

  beforeEach(() => {
    hoisted.sendEvent.mockClear();
  });

  afterEach(() => {
    cleanup();
  });

  it("always renders the chat FAB", () => {
    renderFloatingActionButton();

    expect(
      screen.getByRole("button", { name: "Ask Anarlog anything" }),
    ).not.toBeNull();
  });

  it("keeps chat for states that used to show other FAB actions", () => {
    const cases: Array<
      Partial<React.ComponentProps<typeof FloatingActionButton>>
    > = [
      { allowListening: false },
      { audioExists: true, currentView: { type: "transcript" } },
      { currentView: { type: "enhanced", id: "note-1" } },
      { skipReason: "Not enough words recorded (3/5 minimum)" },
    ];

    for (const props of cases) {
      const view = renderFloatingActionButton(props);

      expect(
        screen.getByRole("button", { name: "Ask Anarlog anything" }),
      ).not.toBeNull();
      expect(
        screen.queryByRole("button", { name: "Start listening" }),
      ).toBeNull();
      expect(
        screen.queryByRole("button", { name: "Generate summary" }),
      ).toBeNull();
      expect(screen.queryByRole("status")).toBeNull();

      view.unmount();
    }
  });

  it("opens chat from the FAB", () => {
    renderFloatingActionButton();

    fireEvent.click(
      screen.getByRole("button", { name: "Ask Anarlog anything" }),
    );

    expect(hoisted.sendEvent).toHaveBeenCalledWith({ type: "OPEN" });
  });
});
