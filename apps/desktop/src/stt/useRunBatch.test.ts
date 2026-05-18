import { describe, expect, test } from "vitest";

import { getBatchProvider, getSessionSpeakerCount } from "./useRunBatch";

describe("getBatchProvider", () => {
  test("maps pyannote to the batch transcription provider", () => {
    expect(getBatchProvider("pyannote", "parakeet-tdt-0.6b-v3")).toBe(
      "pyannote",
    );
  });

  test("keeps openai mapped to the batch transcription provider", () => {
    expect(getBatchProvider("openai", "gpt-4o-transcribe")).toBe("openai");
  });

  test("maps local soniqo models to soniqo batch provider", () => {
    expect(getBatchProvider("hyprnote", "soniqo-parakeet-batch")).toBe(
      "soniqo",
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
