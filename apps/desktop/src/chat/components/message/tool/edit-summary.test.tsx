import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const tabState = vi.hoisted(() => ({
  close: vi.fn(),
  tabs: [] as Array<Record<string, unknown>>,
}));

vi.mock("streamdown", () => ({
  Streamdown: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: {
    getState: () => tabState,
  },
}));

import { ToolEditSummary } from "./edit-summary";

import { usePendingEditStore } from "~/chat/tools/pending-edit-store";

const part = {
  type: "tool-edit_summary",
  toolCallId: "tool-call-1",
  state: "input-available",
  input: { content: "Updated summary" },
} as const;

describe("ToolEditSummary", () => {
  beforeEach(() => {
    cleanup();
    vi.clearAllMocks();
    usePendingEditStore.setState({ edits: new Map() });
    tabState.tabs = [
      {
        active: true,
        pinned: false,
        requestId: "tool-call-1",
        slotId: "review-slot",
        type: "edit",
      },
    ];
  });

  it.each([
    ["Decline", false],
    ["Apply to summary", true],
  ])("resolves %s from the chat card", (label, approved) => {
    const resolve = vi.fn();
    usePendingEditStore.getState().addEdit({
      requestId: "tool-call-1",
      sessionId: "session-1",
      enhancedNoteId: "summary-1",
      currentContent: "Current summary",
      proposedContent: "Updated summary",
      resolve,
    });

    render(<ToolEditSummary part={part} />);
    fireEvent.click(screen.getByRole("button", { name: label }));

    expect(resolve).toHaveBeenCalledWith(approved);
    expect(tabState.close).toHaveBeenCalledWith(tabState.tabs[0]);
    expect(usePendingEditStore.getState().edits.has("tool-call-1")).toBe(false);
  });

  it("hides review actions when the edit is no longer pending", () => {
    render(<ToolEditSummary part={part} />);

    expect(screen.queryByRole("button", { name: "Decline" })).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Apply to summary" }),
    ).toBeNull();
  });
});
