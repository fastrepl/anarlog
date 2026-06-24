import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  cleanup,
  fireEvent,
  render,
  renderHook,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const hoisted = vi.hoisted(() => ({
  store: null as ReturnType<typeof makeStore> | null,
  userId: "self",
  generateText: vi.fn(),
  showTransientToast: vi.fn(),
}));

vi.mock("ai", async (importOriginal) => ({
  ...(await importOriginal<typeof import("ai")>()),
  generateText: hoisted.generateText,
}));

vi.mock("@hypr/plugin-template", () => ({
  commands: {
    renderCustom: vi.fn(async (template: string) => ({
      status: "ok",
      data: template,
    })),
  },
}));

vi.mock("~/ai/hooks", () => ({
  useLanguageModel: () => ({ id: "model-1" }),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  UI: {
    useStore: () => hoisted.store,
    useTable: () => ({}),
    useValue: () => hoisted.userId,
  },
}));

import { buildPastSessionNotes, usePastSessionNotes } from "./past-notes";
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
  showTransientToast: hoisted.showTransientToast,
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (selector: (state: Record<string, never>) => unknown) =>
    selector({}),
}));

vi.mock("~/stt/useRunBatch", () => ({
  useRunBatch: () => vi.fn(),
  isStoppedTranscriptionError: vi.fn(() => false),
}));

beforeEach(() => {
  hoisted.store = null;
  hoisted.userId = "self";
  hoisted.generateText.mockReset();
  hoisted.showTransientToast.mockReset();
});

afterEach(() => {
  cleanup();
});

describe("insights regeneration", () => {
  it("regenerates compiled insights from the insights panel", () => {
    const regenerate = vi.fn();

    render(
      <PostSessionAccessory
        sessionId="session-1"
        isTranscriptExpanded
        activeTab="insights"
        pastNotes={[
          {
            sessionId: "session-0",
            title: "Weekly Product Sync",
            dateLabel: "May 28, 2026",
            summary: "Ship the transcript panel.",
            isGenerating: false,
          },
        ]}
        onRegenerateInsights={regenerate}
      />,
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Regenerate insights" }),
    );

    expect(regenerate).toHaveBeenCalledTimes(1);
  });

  it("disables the regenerate action while insights are generating", () => {
    render(
      <PostSessionAccessory
        sessionId="session-1"
        isTranscriptExpanded
        activeTab="insights"
        pastNotes={[
          {
            sessionId: "session-0",
            title: "Weekly Product Sync",
            dateLabel: "May 28, 2026",
            summary: "Ship the transcript panel.",
            isGenerating: true,
          },
        ]}
        onRegenerateInsights={vi.fn()}
      />,
    );

    expect(
      screen.getByRole("button", { name: "Regenerate insights" }),
    ).toHaveProperty("disabled", true);
  });

  it("disables regeneration while another insight source is generating", () => {
    const regenerate = vi.fn();

    render(
      <PostSessionAccessory
        sessionId="session-1"
        isTranscriptExpanded
        activeTab="insights"
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
        onRegenerateInsights={regenerate}
      />,
    );

    const button = screen.getByRole("button", {
      name: "Regenerate insights",
    });

    expect(button).toHaveProperty("disabled", true);
    fireEvent.click(button);
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
          raw_md: "Raw note text should not feed insights.",
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
      enhanced_notes: {
        previous_summary: {
          session_id: "previous",
          content: "Alex committed to send pricing by Friday.",
          position: 0,
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

  it("recovers from failed related meeting fact regeneration", async () => {
    hoisted.store = makeStore({
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
          raw_md: "Raw note text should not feed insights.",
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
      enhanced_notes: {
        previous_summary: {
          session_id: "previous",
          content: "Alex committed to send pricing by Friday.",
          position: 0,
        },
      },
    });
    const first = buildPastSessionNotes(hoisted.store, "current", "self");
    const request = first.requests[0]!;
    hoisted.store.setRow("session_key_facts", "previous", {
      user_id: "self",
      session_id: "previous",
      created_at: "2026-05-28T11:00:00.000Z",
      updated_at: "2026-05-28T11:00:00.000Z",
      content: "Alex committed to send pricing by Friday.",
      source_hash: request.sourceHash,
    });
    hoisted.generateText.mockRejectedValueOnce(new Error("timed out"));
    hoisted.generateText.mockResolvedValueOnce({
      output: {
        facts: ["Alex will send pricing by Friday."],
      },
    });
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const queryClient = new QueryClient({
      defaultOptions: {
        mutations: {
          retry: false,
        },
      },
    });
    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );

    const { result } = renderHook(
      () => usePastSessionNotes("current", { enabled: true }),
      { wrapper },
    );

    act(() => {
      result.current.regenerate("previous");
    });

    await waitFor(() => {
      expect(hoisted.showTransientToast).toHaveBeenCalledWith({
        id: "past-note-key-facts-error",
        description: "Could not generate meeting insights. Try again.",
        variant: "error",
      });
    });
    expect(consoleError).toHaveBeenCalledWith(
      "Failed to generate meeting insights",
      expect.any(Error),
    );
    await waitFor(() => {
      expect(result.current.isGenerating).toBe(false);
    });

    act(() => {
      result.current.regenerate("previous");
    });

    await waitFor(() => {
      expect(hoisted.generateText).toHaveBeenCalledTimes(2);
    });
    expect(hoisted.generateText.mock.calls[1]?.[0]).toMatchObject({
      timeout: {
        totalMs: 30_000,
      },
    });
    consoleError.mockRestore();
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
    transaction: (callback: () => void) => {
      callback();
    },
  } as any;
}
