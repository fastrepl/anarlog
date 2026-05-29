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
  ChatCTA: () => <button type="button">Ask Anarlog anything</button>,
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
      screen.queryByRole("button", { name: "Ask Anarlog anything" }),
    ).not.toBeNull();
  });

  it("shows the chat FAB on enhanced summary views", () => {
    hoisted.currentTab = { type: "enhanced", id: "note-1" };

    render(<FloatingActionButton tab={tab} />);

    expect(
      screen.queryByRole("button", { name: "Ask Anarlog anything" }),
    ).not.toBeNull();
  });

  it("keeps the chat FAB mounted as a peek while hidden and reveals it from the hover zone", () => {
    render(<FloatingActionButton hidden tab={tab} />);

    const wrapper = screen.getByText("Ask Anarlog anything").parentElement;
    const hoverZone = wrapper?.parentElement;

    expect(hoverZone?.className).toContain("group");
    expect(hoverZone?.className).toContain("pointer-events-auto");
    expect(hoverZone?.className).toContain("max-w-[calc(100%-2rem)]");
    expect(hoverZone?.className).not.toContain("w-96");
    expect(wrapper?.getAttribute("aria-hidden")).toBe("true");
    expect(wrapper?.className).toContain("pointer-events-none");
    expect(wrapper?.className).toContain(
      "translate-y-[var(--floating-fab-tuck-offset)]",
    );
    expect(wrapper?.style.getPropertyValue("--floating-fab-tuck-offset")).toBe(
      "calc(100% - 0.5rem + 18px)",
    );
    expect(wrapper?.className).toContain("group-hover:pointer-events-auto");
    expect(wrapper?.className).toContain("group-hover:translate-y-0");
  });

  it("tucks the listen FAB near the editor caret instead of scroll state", () => {
    hoisted.hasTranscript = false;
    hoisted.isCaretNearBottom = true;

    render(<FloatingActionButton tab={tab} />);

    const wrapper = screen.getByText("Start listening").parentElement;

    expect(wrapper?.getAttribute("aria-hidden")).toBe("true");
    expect(wrapper?.style.getPropertyValue("--floating-fab-tuck-offset")).toBe(
      "calc(100% - 0.5rem + 18px)",
    );
  });

  it("keeps the listen FAB popped up when only scroll hidden is set", () => {
    hoisted.hasTranscript = false;

    render(<FloatingActionButton hidden tab={tab} />);

    const wrapper = screen.getByText("Start listening").parentElement;

    expect(wrapper?.getAttribute("aria-hidden")).toBe("false");
    expect(wrapper?.style.getPropertyValue("--floating-fab-tuck-offset")).toBe(
      "0px",
    );
  });

  it("shows a skip reason in the FAB slot instead of the chat FAB", () => {
    render(
      <FloatingActionButton
        tab={tab}
        skipReason="Not enough words recorded (3/5 minimum)"
      />,
    );

    const status = screen.getByRole("status");

    expect(status.textContent).toBe("Not enough words recorded (3/5 minimum)");
    expect(status.className).toContain("text-red-400");
    expect(status.parentElement?.className).toContain("pb-4");
    expect(
      screen.queryByRole("button", { name: "Ask Anarlog anything" }),
    ).toBeNull();
  });

  it("keeps a skip reason visible even when the FAB is tucked", () => {
    render(
      <FloatingActionButton
        hidden
        tab={tab}
        skipReason="Not enough words recorded (3/5 minimum)"
      />,
    );

    const status = screen.getByRole("status");

    expect(status.className).toContain("translate-y-0");
    expect(status.parentElement?.className).not.toContain("group");
  });
});
