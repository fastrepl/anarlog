import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { buildPastSessionNotes } from "./past-notes";
import { PostSessionAccessory } from "./post-session";

vi.mock("@hypr/plugin-fs-sync", () => ({
  commands: {
    audioPath: vi.fn(),
  },
}));

vi.mock("@hypr/ui/components/ui/button", () => ({
  Button: ({
    children,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button {...props}>{children}</button>
  ),
}));

vi.mock("@hypr/ui/components/ui/spinner", () => ({
  Spinner: () => <div data-testid="spinner" />,
}));

vi.mock("@hypr/ui/components/ui/tooltip", () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  TooltipTrigger: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
  TooltipContent: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
}));

vi.mock("~/audio-player", () => ({
  Timeline: () => <div data-testid="timeline" />,
  TimelineShell: ({ children }: { children?: React.ReactNode }) => (
    <div>{children}</div>
  ),
  TimelineMeta: ({ children }: { children?: React.ReactNode }) => (
    <div>{children}</div>
  ),
  useAudioPlayer: () => ({
    audioExists: false,
    deleteRecording: vi.fn(),
    isDeletingRecording: false,
  }),
}));

vi.mock("~/session/components/note-input/transcript", () => ({
  Transcript: () => <div data-testid="transcript" />,
}));

vi.mock("~/session/components/note-input/transcript/export-data", () => ({
  useTranscriptExportSegments: () => ({ data: [], isLoading: false }),
  formatTranscriptExportSegments: () => "",
}));

vi.mock("~/session/components/note-input/transcript/state", () => ({
  useTranscriptScreen: () => ({ kind: "empty" }),
}));

vi.mock("~/sidebar/toast/transient", () => ({
  showTransientToast: vi.fn(),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (selector: (state: Record<string, never>) => unknown) =>
    selector({}),
}));

vi.mock("~/stt/useRunBatch", () => ({
  useRunBatch: () => vi.fn(),
  isStoppedTranscriptionError: vi.fn(() => false),
}));

afterEach(() => {
  cleanup();
});

describe("past note regeneration", () => {
  it("regenerates a selected past note from its timeline row", () => {
    const regenerate = vi.fn();

    render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio={false}
        hasTranscript
        isTranscriptExpanded
        activeTab="past_notes"
        pastNotes={[
          {
            sessionId: "session-0",
            title: "Weekly Product Sync",
            dateLabel: "May 28, 2026",
            summary: "Ship the transcript panel.",
            isGenerating: false,
          },
        ]}
        onRegeneratePastNote={regenerate}
      />,
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Regenerate past note summary" }),
    );

    expect(regenerate).toHaveBeenCalledWith("session-0");
  });

  it("disables the row regenerate action while that note is generating", () => {
    render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio={false}
        hasTranscript
        isTranscriptExpanded
        activeTab="past_notes"
        pastNotes={[
          {
            sessionId: "session-0",
            title: "Weekly Product Sync",
            dateLabel: "May 28, 2026",
            summary: "Ship the transcript panel.",
            isGenerating: true,
          },
        ]}
        onRegeneratePastNote={vi.fn()}
      />,
    );

    expect(
      screen.getByRole("button", { name: "Regenerate past note summary" }),
    ).toHaveProperty("disabled", true);
  });

  it("disables row regenerate actions while another note is generating", () => {
    const regenerate = vi.fn();

    render(
      <PostSessionAccessory
        sessionId="session-1"
        hasAudio={false}
        hasTranscript
        isTranscriptExpanded
        activeTab="past_notes"
        pastNotes={[
          {
            sessionId: "session-0",
            title: "Weekly Product Sync",
            dateLabel: "May 28, 2026",
            summary: "Ship the transcript panel.",
            isGenerating: false,
            isRegenerateDisabled: true,
          },
          {
            sessionId: "session-2",
            title: "Design Review",
            dateLabel: "May 20, 2026",
            summary: "Review final mocks.",
            isGenerating: true,
            isRegenerateDisabled: true,
          },
        ]}
        onRegeneratePastNote={regenerate}
      />,
    );

    const buttons = screen.getAllByRole("button", {
      name: "Regenerate past note summary",
    });

    expect(buttons).toHaveLength(2);
    expect(buttons[0]).toHaveProperty("disabled", true);
    expect(buttons[1]).toHaveProperty("disabled", true);
    fireEvent.click(buttons[0]!);
    expect(regenerate).not.toHaveBeenCalled();
  });

  it("keeps regeneration requests for saved past note facts", () => {
    const store = makeStore({
      sessions: {
        current: {
          title: "Weekly Product Sync",
          created_at: "2026-06-03T10:00:00.000Z",
          event_json: "",
          raw_md: "",
        },
        previous: {
          title: "Weekly Product Sync",
          created_at: "2026-05-28T10:00:00.000Z",
          event_json: "",
          raw_md: "Alex committed to send pricing by Friday.",
        },
      },
      mapping_session_participant: {
        current_alex: {
          session_id: "current",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
        previous_alex: {
          session_id: "previous",
          human_id: "alex",
          user_id: "self",
          source: "auto",
        },
      },
    });
    const first = buildPastSessionNotes(store, "current", "self");
    const request = first.requests[0]!;

    store.setRow("session_key_facts", "previous", {
      user_id: "self",
      session_id: "previous",
      created_at: "2026-05-28T11:00:00.000Z",
      updated_at: "2026-05-28T11:00:00.000Z",
      content: "Alex committed to send pricing by Friday.",
      source_hash: request.sourceHash,
    });

    const second = buildPastSessionNotes(store, "current", "self");

    expect(second.missing).toHaveLength(0);
    expect(second.requests.map((request) => request.sessionId)).toEqual([
      "previous",
    ]);
  });
});

function makeStore(
  tables: Record<string, Record<string, Record<string, any>>>,
) {
  return {
    getRow: (tableId: string, rowId: string) => tables[tableId]?.[rowId] ?? {},
    getCell: (tableId: string, rowId: string, cellId: string) =>
      tables[tableId]?.[rowId]?.[cellId],
    forEachRow: (
      tableId: string,
      callback: (rowId: string, forEachCell: unknown) => void,
    ) => {
      for (const rowId of Object.keys(tables[tableId] ?? {})) {
        callback(rowId, () => {});
      }
    },
    setRow: (tableId: string, rowId: string, row: Record<string, any>) => {
      tables[tableId] = {
        ...(tables[tableId] ?? {}),
        [rowId]: row,
      };
    },
  } as any;
}
