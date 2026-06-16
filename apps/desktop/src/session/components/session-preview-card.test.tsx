import { cleanup, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  contentOpen: false,
  enhancedNoteIds: [] as string[],
  useCell: vi.fn(
    (_table: string, _rowId: string, cellId: string) =>
      (
        ({
          title: "Design sync",
          raw_md: "Discussed launch readiness and owner follow-ups.",
          created_at: "2024-01-15T10:30:00.000Z",
          event_json: undefined,
        }) as Record<string, string | undefined>
      )[cellId],
  ),
  useEnhancedNote: vi.fn(() => ({ content: undefined, title: undefined })),
  useEnhancedNotes: vi.fn(() => mocks.enhancedNoteIds),
  useResultTable: vi.fn(() => ({})),
  useSliceRowIds: vi.fn(() => []),
}));

vi.mock("@hypr/editor/markdown", () => ({
  isValidContent: () => false,
  json2md: () => "",
}));

vi.mock("@hypr/editor/node-views", () => ({
  parseImageMetadata: () => ({ editorWidth: undefined, title: undefined }),
}));

vi.mock("@hypr/ui/components/ui/hover-card", () => ({
  HoverCard: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  HoverCardContent: ({ children }: { children: ReactNode }) =>
    mocks.contentOpen ? (
      <div data-testid="preview-content">{children}</div>
    ) : null,
  HoverCardTrigger: ({ children }: { children: ReactNode }) => (
    <div>{children}</div>
  ),
}));

vi.mock("~/search/contexts/engine/utils", () => ({
  extractPlainText: (value: string) => value,
}));

vi.mock("~/session/components/streamdown", () => ({
  streamdownComponents: {},
}));

vi.mock("~/session/hooks/useEnhancedNotes", () => ({
  useEnhancedNote: mocks.useEnhancedNote,
  useEnhancedNotes: mocks.useEnhancedNotes,
}));

vi.mock("~/store/tinybase/store/main", () => ({
  INDEXES: {
    sessionParticipantsBySession: "sessionParticipantsBySession",
  },
  QUERIES: {
    sessionParticipantsWithDetails: "sessionParticipantsWithDetails",
  },
  STORE_ID: "main",
  UI: {
    useCell: mocks.useCell,
    useResultTable: mocks.useResultTable,
    useSliceRowIds: mocks.useSliceRowIds,
  },
}));

import { SessionPreviewCard } from "./session-preview-card";

describe("SessionPreviewCard", () => {
  beforeEach(() => {
    mocks.contentOpen = false;
    mocks.enhancedNoteIds = [];
    mocks.useCell.mockClear();
    mocks.useEnhancedNote.mockClear();
    mocks.useEnhancedNotes.mockClear();
    mocks.useResultTable.mockClear();
    mocks.useSliceRowIds.mockClear();
  });

  afterEach(() => {
    cleanup();
  });

  it("defers preview data subscriptions until the hover content mounts", () => {
    const { rerender } = render(
      <SessionPreviewCard sessionId="session-1" side="right">
        <button type="button">Open note</button>
      </SessionPreviewCard>,
    );

    expect(screen.getByRole("button", { name: "Open note" })).toBeTruthy();
    expect(mocks.useCell).not.toHaveBeenCalled();
    expect(mocks.useEnhancedNotes).not.toHaveBeenCalled();
    expect(mocks.useSliceRowIds).not.toHaveBeenCalled();

    mocks.contentOpen = true;
    rerender(
      <SessionPreviewCard sessionId="session-1" side="right">
        <button type="button">Open note</button>
      </SessionPreviewCard>,
    );

    expect(screen.getByTestId("preview-content")).toBeTruthy();
    expect(screen.getByText("Design sync")).toBeTruthy();
    expect(mocks.useCell).toHaveBeenCalled();
    expect(mocks.useEnhancedNotes).toHaveBeenCalledWith("session-1");
    expect(mocks.useSliceRowIds).toHaveBeenCalledWith(
      "sessionParticipantsBySession",
      "session-1",
      "main",
    );
  });
});
