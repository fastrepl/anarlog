import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { getBatchProvider, getSessionSpeakerCount } from "./useRunBatch";
import { useRunBatch } from "./useRunBatch";

const {
  startTranscriptionMock,
  useListenerMock,
  useStoreMock,
  useIndexesMock,
  useValuesMock,
  useSTTConnectionMock,
  useConfigValueMock,
  settingsUseStoreMock,
  deleteProcessedAudioForRetentionMock,
  saveMock,
  idMock,
} = vi.hoisted(() => ({
  startTranscriptionMock: vi.fn(),
  useListenerMock: vi.fn(),
  useStoreMock: vi.fn(),
  useIndexesMock: vi.fn(),
  useValuesMock: vi.fn(),
  useSTTConnectionMock: vi.fn(),
  useConfigValueMock: vi.fn(),
  settingsUseStoreMock: vi.fn(),
  deleteProcessedAudioForRetentionMock: vi.fn(),
  saveMock: vi.fn(),
  idMock: vi.fn(),
}));

vi.mock("./contexts", () => ({
  useListener: useListenerMock,
}));

vi.mock("./useKeywords", () => ({
  useKeywords: vi.fn(() => []),
}));

vi.mock("./useSTTConnection", () => ({
  useSTTConnection: useSTTConnectionMock,
}));

vi.mock("~/services/audio-retention", () => ({
  deleteProcessedAudioForRetention: deleteProcessedAudioForRetentionMock,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: useConfigValueMock,
}));

vi.mock("~/shared/utils", () => ({
  id: idMock,
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  INDEXES: {
    transcriptBySession: "transcriptBySession",
  },
  UI: {
    useStore: useStoreMock,
    useIndexes: useIndexesMock,
    useValues: useValuesMock,
  },
}));

vi.mock("~/store/tinybase/store/settings", () => ({
  STORE_ID: "settings",
  UI: {
    useStore: settingsUseStoreMock,
  },
}));

vi.mock("~/store/tinybase/store/save", () => ({
  save: saveMock,
}));

function createStore() {
  const tables = {
    sessions: new Map<string, Record<string, unknown>>([
      ["session-1", { raw_md: "Existing memo" }],
    ]),
    transcripts: new Map<string, Record<string, unknown>>(),
    mapping_session_participant: new Map<string, Record<string, unknown>>(),
  };

  return {
    tables,
    forEachRow: (
      tableId: keyof typeof tables,
      callback: (rowId: string) => void,
    ) => {
      for (const rowId of tables[tableId].keys()) callback(rowId);
    },
    getCell: (tableId: keyof typeof tables, rowId: string, cellId: string) =>
      tables[tableId].get(rowId)?.[cellId],
    setCell: (
      tableId: keyof typeof tables,
      rowId: string,
      cellId: string,
      value: unknown,
    ) => {
      tables[tableId].set(rowId, {
        ...tables[tableId].get(rowId),
        [cellId]: value,
      });
    },
    setRow: (
      tableId: keyof typeof tables,
      rowId: string,
      row: Record<string, unknown>,
    ) => {
      tables[tableId].set(rowId, row);
    },
    delRow: (tableId: keyof typeof tables, rowId: string) => {
      tables[tableId].delete(rowId);
    },
    transaction: (callback: () => void) => callback(),
  };
}

describe("getBatchProvider", () => {
  test("maps pyannote to the batch transcription provider", () => {
    expect(getBatchProvider("pyannote", "parakeet-tdt-0.6b-v3")).toBe(
      "pyannote",
    );
  });

  test("keeps openai mapped to the batch transcription provider", () => {
    expect(getBatchProvider("openai", "gpt-4o-transcribe")).toBe("openai");
  });

  test("keeps cartesia mapped to the batch transcription provider", () => {
    expect(getBatchProvider("cartesia", "ink-2")).toBe("cartesia");
  });

  test("maps Cloudflare Workers AI to the Deepgram-compatible batch provider", () => {
    expect(getBatchProvider("cloudflare_workers_ai", "nova-3")).toBe(
      "deepgram",
    );
  });

  test("maps local soniqo models to soniqo batch provider", () => {
    expect(getBatchProvider("hyprnote", "soniqo-parakeet-batch")).toBe(
      "soniqo",
    );
  });
});

describe("useRunBatch", () => {
  beforeEach(() => {
    vi.clearAllMocks();

    let nextId = 0;
    idMock.mockImplementation(() => `generated-${++nextId}`);
    saveMock.mockResolvedValue(undefined);
    deleteProcessedAudioForRetentionMock.mockResolvedValue(undefined);
    useListenerMock.mockImplementation((selector) =>
      selector({ startTranscription: startTranscriptionMock }),
    );
    useStoreMock.mockReturnValue(createStore());
    useIndexesMock.mockReturnValue({
      getSliceRowIds: vi.fn(() => []),
    });
    useValuesMock.mockReturnValue({ user_id: "user-1" });
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "deepgram",
        model: "nova-3",
        baseUrl: "https://api.deepgram.com/v1/listen",
        apiKey: "test-key",
      },
    });
    useConfigValueMock.mockImplementation((key) =>
      key === "ai_language" ? "en" : [],
    );
    settingsUseStoreMock.mockReturnValue({ id: "settings-store" });
  });

  test("saves once after streamed default batch persists finish", async () => {
    startTranscriptionMock.mockImplementation(async (_params, options) => {
      options.handlePersist(
        [{ text: "hello", start_ms: 0, end_ms: 100, channel: 0 }],
        [],
      );
      options.handlePersist(
        [{ text: "world", start_ms: 100, end_ms: 200, channel: 0 }],
        [],
      );
    });

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav");
    });

    expect(saveMock).toHaveBeenCalledTimes(1);
    expect(deleteProcessedAudioForRetentionMock).toHaveBeenCalledTimes(1);
    expect(saveMock.mock.invocationCallOrder[0]).toBeLessThan(
      deleteProcessedAudioForRetentionMock.mock.invocationCallOrder[0],
    );
  });

  test("does not save for custom batch persist handlers", async () => {
    const handlePersist = vi.fn();
    startTranscriptionMock.mockImplementation(async (_params, options) => {
      options.handlePersist(
        [{ text: "custom", start_ms: 0, end_ms: 100, channel: 0 }],
        [],
      );
    });

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav", { handlePersist });
    });

    expect(handlePersist).toHaveBeenCalledTimes(1);
    expect(saveMock).not.toHaveBeenCalled();
  });

  test("flushes default batch persists before rethrowing transcription errors", async () => {
    startTranscriptionMock.mockImplementation(async (_params, options) => {
      options.handlePersist(
        [{ text: "partial", start_ms: 0, end_ms: 100, channel: 0 }],
        [],
      );
      throw new Error("provider failed");
    });

    const { result } = renderHook(() => useRunBatch("session-1"));

    await expect(
      act(async () => {
        await result.current("/tmp/session.wav");
      }),
    ).rejects.toThrow("provider failed");

    expect(saveMock).toHaveBeenCalledTimes(1);
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
  });

  test("passes selected transcription languages to batch transcription", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "hyprnote",
        model: "soniqo-parakeet-batch",
        baseUrl: "soniqo://local",
        apiKey: "",
      },
    });
    useConfigValueMock.mockImplementation((key) =>
      key === "ai_language" ? "de" : ["en"],
    );
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav");
    });

    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: "soniqo",
        model: "soniqo-parakeet-batch",
        languages: ["de", "en"],
      }),
      expect.any(Object),
    );
  });
});

describe("getSessionSpeakerCount", () => {
  test("counts distinct session participants plus the current user", () => {
    const rows = new Map([
      ["mapping-1", { session_id: "session-1", human_id: "human-a" }],
      ["mapping-2", { session_id: "session-1", human_id: "human-a" }],
      ["mapping-3", { session_id: "session-1", human_id: "human-b" }],
      ["mapping-4", { session_id: "other-session", human_id: "human-c" }],
    ]);
    const store = {
      forEachRow: (_table: string, callback: (rowId: string) => void) => {
        for (const rowId of rows.keys()) callback(rowId);
      },
      getCell: (_table: string, rowId: string, cellId: string) =>
        rows.get(rowId)?.[cellId as "session_id" | "human_id"],
    };

    expect(getSessionSpeakerCount(store as any, "session-1", "self")).toBe(3);
  });

  test("returns undefined until at least two speakers are known", () => {
    const rows = new Map([
      ["mapping-1", { session_id: "session-1", human_id: "human-a" }],
    ]);
    const store = {
      forEachRow: (_table: string, callback: (rowId: string) => void) => {
        for (const rowId of rows.keys()) callback(rowId);
      },
      getCell: (_table: string, rowId: string, cellId: string) =>
        rows.get(rowId)?.[cellId as "session_id" | "human_id"],
    };

    expect(getSessionSpeakerCount(store as any, "session-1", null)).toBe(
      undefined,
    );
  });
});
