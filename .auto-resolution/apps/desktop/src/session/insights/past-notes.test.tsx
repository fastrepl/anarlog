import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const hoisted = vi.hoisted(() => ({
  data: null as ReturnType<typeof makeData> | null,
  executeTransaction: vi.fn(
    (_statements: Array<{ sql: string; params: unknown[] }>) =>
      Promise.resolve([1]),
  ),
  generateText: vi.fn(),
  toastError: vi.fn(),
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

vi.mock("@hypr/ui/components/ui/toast", () => ({
  sonnerToast: { error: hoisted.toastError },
}));

vi.mock("~/ai/hooks", () => ({
  useLanguageModel: () => ({ id: "model-1" }),
}));

vi.mock("~/db", () => ({
  executeTransaction: hoisted.executeTransaction,
  useLiveQuery: (options: {
    sql: string;
    enabled?: boolean;
    mapRows: (rows: Array<Record<string, unknown>>) => unknown;
  }) => {
    if (options.enabled === false) return {};
    const data = hoisted.data ?? {
      sessions: {},
      participants: [],
      enhancedNotes: [],
      keyFacts: {},
    };
    const rows = options.sql.includes("FROM sessions")
      ? Object.values(data.sessions)
      : options.sql.includes("FROM session_participants")
        ? data.participants
        : options.sql.includes("kind = 'enhanced_note'")
          ? data.enhancedNotes
          : Object.values(data.keyFacts);
    return { data: options.mapRows(rows) };
  },
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: (_key: string, operation: () => Promise<unknown>) =>
    operation(),
}));

import {
  buildPastSessionNotes,
  type PastSessionNotesData,
  usePastSessionNotes,
} from "./past-notes";

beforeEach(() => {
  hoisted.data = null;
  hoisted.executeTransaction.mockClear();
  hoisted.generateText.mockReset();
  hoisted.toastError.mockReset();
});

afterEach(() => {
  cleanup();
});

describe("insights regeneration", () => {
  it("keeps regeneration requests for saved past note facts", () => {
    const data = makeData({
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
    const first = buildPastSessionNotes(data, "current", "self");
    const request = first.requests[0]!;

    data.keyFacts.previous = {
      user_id: "self",
      session_id: "previous",
      created_at: "2026-05-28T11:00:00.000Z",
      updated_at: "2026-05-28T11:00:00.000Z",
      content: "Alex committed to send pricing by Friday.",
      source_hash: request.sourceHash,
    };

    const second = buildPastSessionNotes(data, "current", "self");

    expect(second.missing).toHaveLength(0);
    expect(second.requests.map((request) => request.sessionId)).toEqual([
      "previous",
    ]);
  });

  it("recovers from failed related meeting fact regeneration", async () => {
    hoisted.data = makeData({
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
    const first = buildPastSessionNotes(hoisted.data, "current", "self");
    const request = first.requests[0]!;
    hoisted.data.keyFacts.previous = {
      user_id: "self",
      session_id: "previous",
      created_at: "2026-05-28T11:00:00.000Z",
      updated_at: "2026-05-28T11:00:00.000Z",
      content: "Alex committed to send pricing by Friday.",
      source_hash: request.sourceHash,
    };
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
      expect(hoisted.toastError).toHaveBeenCalledWith(
        "Could not generate meeting insights. Try again.",
        {
          id: "past-note-key-facts-error",
        },
      );
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
    await waitFor(() =>
      expect(hoisted.executeTransaction).toHaveBeenCalledTimes(1),
    );
    const statements = hoisted.executeTransaction.mock.calls[0][0];
    expect(statements).toHaveLength(2);
    expect(statements[0].sql).toContain("UPDATE session_documents");
    expect(statements[0].sql).toContain("session_id = ?");
    expect(statements[1].sql).toContain("INSERT INTO session_documents");
    expect(statements[1].sql).toContain("ON CONFLICT(id) DO UPDATE");
    consoleError.mockRestore();
  });
});

function makeData(
  tables: Record<string, Record<string, Record<string, any>>>,
): PastSessionNotesData {
  return {
    sessions: Object.fromEntries(
      Object.entries(tables.sessions ?? {}).map(([id, row]) => [
        id,
        {
          id,
          user_id: String(row.user_id ?? "self"),
          title: String(row.title ?? ""),
          created_at: String(row.created_at ?? ""),
          event_json: String(row.event_json ?? ""),
        },
      ]),
    ),
    participants: Object.values(tables.mapping_session_participant ?? {}).map(
      (row) => ({
        session_id: String(row.session_id ?? ""),
        human_id: String(row.human_id ?? ""),
        user_id: String(row.user_id ?? ""),
        source: String(row.source ?? ""),
        name: String(row.name ?? row.human_id ?? ""),
      }),
    ),
    enhancedNotes: Object.values(tables.enhanced_notes ?? {}).map((row) => ({
      session_id: String(row.session_id ?? ""),
      content: String(row.content ?? ""),
      position: Number(row.position ?? 0),
    })),
    keyFacts: Object.fromEntries(
      Object.values(tables.session_key_facts ?? {}).map((row) => [
        String(row.session_id ?? ""),
        {
          session_id: String(row.session_id ?? ""),
          user_id: String(row.user_id ?? ""),
          created_at: String(row.created_at ?? ""),
          updated_at: String(row.updated_at ?? ""),
          content: String(row.content ?? ""),
          source_hash: String(row.source_hash ?? ""),
        },
      ]),
    ),
  };
}
