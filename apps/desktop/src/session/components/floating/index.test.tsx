import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { FloatingActionButton } from "./index";

import type { Tab } from "~/store/zustand/tabs";

const hoisted = vi.hoisted(() => ({
  currentTab: { type: "raw" } as
    | { type: "raw" }
    | {
        type: "enhanced";
        id: string;
      },
  hasTranscript: true,
  isCaretNearBottom: false,
  sessionMode: "inactive",
}));

vi.mock("./listen", () => ({
  ListenButton: () => <button type="button">Start listening</button>,
}));

vi.mock("~/shared/chat-cta", () => ({
  ChatCTA: () => <button type="button">Ask about this session</button>,
}));

vi.mock("~/session/components/shared", () => ({
  useCurrentNoteTab: () => hoisted.currentTab,
  useHasTranscript: () => hoisted.hasTranscript,
}));

vi.mock("../caret-position-context", () => ({
  useCaretPosition: () => ({
    isCaretNearBottom: hoisted.isCaretNearBottom,
  }),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (
    selector: (state: { getSessionMode: () => string }) => unknown,
  ) =>
    selector({
      getSessionMode: () => hoisted.sessionMode,
    }),
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

  beforeEach(() => {
    hoisted.currentTab = { type: "raw" };
    hoisted.hasTranscript = true;
    hoisted.isCaretNearBottom = false;
    hoisted.sessionMode = "inactive";
  });

  afterEach(() => {
    cleanup();
  });

  it("shows the chat FAB on raw memo view after transcript exists", () => {
    render(<FloatingActionButton tab={tab} />);

    expect(
      screen.queryByRole("button", { name: "Ask about this session" }),
    ).not.toBeNull();
  });

  it("shows the chat FAB on enhanced summary views", () => {
    hoisted.currentTab = { type: "enhanced", id: "note-1" };

    render(<FloatingActionButton tab={tab} />);

    expect(
      screen.queryByRole("button", { name: "Ask about this session" }),
    ).not.toBeNull();
  });

  it("keeps the chat FAB mounted as a peek while hidden and reveals it from the hover zone", () => {
    render(<FloatingActionButton hidden tab={tab} />);

    const wrapper = screen.getByText("Ask about this session").parentElement;
    const hoverZone = wrapper?.parentElement;

    expect(hoverZone?.className).toContain("group");
    expect(hoverZone?.className).toContain("pointer-events-auto");
    expect(wrapper?.getAttribute("aria-hidden")).toBe("true");
    expect(wrapper?.className).toContain("pointer-events-none");
    expect(wrapper?.className).toContain("translate-y-[calc(100%+0.5rem)]");
    expect(wrapper?.className).toContain("group-hover:pointer-events-auto");
    expect(wrapper?.className).toContain("group-hover:translate-y-0");
    expect(wrapper?.className).toContain("opacity-100");
  });
});
