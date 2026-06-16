import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { FloatingActionButton } from "./index";

import type { LLMConnectionStatus } from "~/ai/hooks";
import type { Tab } from "~/store/zustand/tabs";

const hoisted = vi.hoisted(() => ({
  currentTab: { type: "raw" } as
    | { type: "raw" }
    | {
        type: "enhanced";
        id: string;
      },
  hasTranscript: true,
  enhanceTaskStatus: undefined as string | undefined,
  enhancedContent: "Generated summary",
  llmStatus: {
    status: "success",
    providerId: "hyprnote",
    isHosted: true,
  } as LLMConnectionStatus,
  isCaretNearBottom: false,
  sessionMode: "inactive",
  sendEvent: vi.fn(),
}));

vi.mock("./listen", () => ({
  ListenButton: () => <button type="button">Start listening</button>,
}));

vi.mock("~/shared/chat-cta", () => ({
  ChatCTA: () => (
    <button type="button" onClick={() => hoisted.sendEvent({ type: "OPEN" })}>
      Ask Anarlog anything
    </button>
  ),
}));

vi.mock("~/session/components/shared", () => ({
  useCurrentNoteTab: () => hoisted.currentTab,
  useHasTranscript: () => hoisted.hasTranscript,
}));

vi.mock("~/ai/contexts", () => ({
  useAITask: (
    selector: (state: {
      tasks: Record<string, { status: string | undefined }>;
    }) => unknown,
  ) =>
    selector({
      tasks: hoisted.enhanceTaskStatus
        ? { "note-1-enhance": { status: hoisted.enhanceTaskStatus } }
        : {},
    }),
}));

vi.mock("~/ai/hooks", () => ({
  useLLMConnectionStatus: () => hoisted.llmStatus,
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  UI: {
    useCell: () => hoisted.enhancedContent,
  },
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
    hoisted.enhanceTaskStatus = undefined;
    hoisted.enhancedContent = "Generated summary";
    hoisted.llmStatus = {
      status: "success",
      providerId: "hyprnote",
      isHosted: true,
    };
    hoisted.isCaretNearBottom = false;
    hoisted.sessionMode = "inactive";
    hoisted.sendEvent.mockClear();
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

  it("hides the chat FAB when the visible summary has a generation issue", () => {
    hoisted.currentTab = { type: "enhanced", id: "note-1" };
    hoisted.enhanceTaskStatus = "error";

    render(<FloatingActionButton tab={tab} />);

    expect(
      screen.queryByRole("button", { name: "Ask Anarlog anything" }),
    ).toBeNull();
  });

  it("hides the chat FAB when the visible summary has a setup issue", () => {
    hoisted.currentTab = { type: "enhanced", id: "note-1" };
    hoisted.enhanceTaskStatus = "idle";
    hoisted.enhancedContent = "";
    hoisted.llmStatus = { status: "pending", reason: "missing_provider" };

    render(<FloatingActionButton tab={tab} />);

    expect(
      screen.queryByRole("button", { name: "Ask Anarlog anything" }),
    ).toBeNull();
  });

  it("shows the chat FAB while an empty enhanced summary is still generating", () => {
    hoisted.currentTab = { type: "enhanced", id: "note-1" };
    hoisted.enhanceTaskStatus = "generating";
    hoisted.enhancedContent = "";
    hoisted.llmStatus = { status: "pending", reason: "missing_provider" };

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
    expect(hoverZone?.className).toContain("z-30");
    expect(hoverZone?.className).toContain("bottom-0");
    expect(hoverZone?.className).not.toContain("-bottom-4");
    expect(hoverZone?.className).toContain("h-32");
    expect(hoverZone?.className).toContain("pb-4");
    expect(hoverZone?.className).toContain("max-w-[calc(100%-2rem)]");
    expect(hoverZone?.className).not.toContain("w-96");
    expect(wrapper?.getAttribute("aria-hidden")).toBe("true");
    expect(wrapper?.className).toContain("pointer-events-none");
    expect(wrapper?.className).toContain("before:-inset-x-8");
    expect(wrapper?.className).toContain("before:-inset-y-8");
    expect(wrapper?.className).toContain(
      "translate-y-[var(--floating-fab-tuck-offset)]",
    );
    expect(wrapper?.style.getPropertyValue("--floating-fab-tuck-offset")).toBe(
      "calc(100% - 0.5rem + 18px)",
    );
    expect(wrapper?.className).toContain("group-hover:pointer-events-auto");
    expect(wrapper?.className).toContain("group-hover:translate-y-0");
    expect(wrapper?.className).toContain("hover:pointer-events-auto");
    expect(wrapper?.className).toContain("hover:translate-y-0");
  });

  it("tucks the chat FAB near the editor caret and reveals it from the hover zone", () => {
    hoisted.isCaretNearBottom = true;

    render(<FloatingActionButton tab={tab} />);

    const wrapper = screen.getByText("Ask Anarlog anything").parentElement;
    const hoverZone = wrapper?.parentElement;

    expect(hoverZone?.className).toContain("group");
    expect(hoverZone?.className).toContain("pointer-events-auto");
    expect(hoverZone?.className).toContain("bottom-0");
    expect(hoverZone?.className).not.toContain("-bottom-4");
    expect(hoverZone?.className).toContain("h-32");
    expect(hoverZone?.className).toContain("pb-4");
    expect(wrapper?.getAttribute("aria-hidden")).toBe("true");
    expect(wrapper?.style.getPropertyValue("--floating-fab-tuck-offset")).toBe(
      "calc(100% - 0.5rem + 18px)",
    );
    expect(wrapper?.className).toContain("group-hover:pointer-events-auto");
    expect(wrapper?.className).toContain("group-hover:translate-y-0");
    expect(wrapper?.className).toContain("hover:pointer-events-auto");
    expect(wrapper?.className).toContain("hover:translate-y-0");
  });

  it("keeps the chat FAB tucked during active meetings", () => {
    hoisted.sessionMode = "active";

    render(<FloatingActionButton tab={tab} />);

    const wrapper = screen.getByText("Ask Anarlog anything").parentElement;
    const hoverZone = wrapper?.parentElement;

    expect(hoverZone?.className).toContain("group");
    expect(hoverZone?.className).toContain("pointer-events-auto");
    expect(hoverZone?.className).toContain("bottom-0");
    expect(hoverZone?.className).not.toContain("-bottom-4");
    expect(hoverZone?.className).toContain("h-32");
    expect(hoverZone?.className).toContain("pb-4");
    expect(wrapper?.getAttribute("aria-hidden")).toBe("false");
    expect(wrapper?.style.getPropertyValue("--floating-fab-tuck-offset")).toBe(
      "calc(100% - 0.5rem + 18px)",
    );
    expect(wrapper?.className).toContain("pointer-events-none");
    expect(wrapper?.className).toContain("group-hover:pointer-events-auto");
    expect(wrapper?.className).toContain("group-hover:translate-y-0");
    expect(wrapper?.className).toContain("hover:pointer-events-auto");
    expect(wrapper?.className).toContain("hover:translate-y-0");
  });

  it("shows the tucked chat FAB during active meetings before transcript exists", () => {
    hoisted.sessionMode = "active";
    hoisted.hasTranscript = false;

    render(<FloatingActionButton tab={tab} />);

    const wrapper = screen.getByText("Ask Anarlog anything").parentElement;

    expect(
      screen.queryByRole("button", { name: "Start listening" }),
    ).toBeNull();
    expect(wrapper?.getAttribute("aria-hidden")).toBe("false");
    expect(wrapper?.style.getPropertyValue("--floating-fab-tuck-offset")).toBe(
      "calc(100% - 0.5rem + 18px)",
    );
  });

  it("opens chat from the tucked active meeting FAB", () => {
    hoisted.sessionMode = "active";

    render(<FloatingActionButton tab={tab} />);

    fireEvent.click(
      screen.getByRole("button", { name: "Ask Anarlog anything" }),
    );

    expect(hoisted.sendEvent).toHaveBeenCalledWith({ type: "OPEN" });
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
