import { beforeEach, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  executeTransaction: vi.fn().mockResolvedValue([1]),
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  liveQueryClient: { execute: mocks.execute },
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: (_key: string, operation: () => Promise<unknown>) =>
    operation(),
}));

import {
  clearCaptureLifecycleMarker,
  loadCaptureLifecycleMarker,
  loadCaptureLifecycleMarkers,
  saveCaptureLifecycleMarker,
  type CaptureLifecycleMarker,
} from "./capture-lifecycle-storage";

const marker: CaptureLifecycleMarker = {
  version: 1,
  phase: "capturing",
  sessionId: "session-1",
  transcriptId: "transcript-1",
  startedAt: 1_000,
  createdAt: "2026-07-24T00:00:00.000Z",
  audioOffsetMs: 20_000,
  preserveExistingTranscript: true,
  ownerUserId: "user-1",
  memo: "memo",
  provider: "anarlog",
  model: "am",
};

beforeEach(() => {
  vi.clearAllMocks();
  mocks.executeTransaction.mockResolvedValue([1]);
});

test("persists the exact capture recovery marker", async () => {
  await saveCaptureLifecycleMarker(marker);

  const statement = mocks.executeTransaction.mock.calls[0]?.[0]?.[0];
  expect(statement.sql).toContain("INSERT INTO app_settings");
  expect(statement.sql).toContain("app_settings.value_json");
  expect(statement.params[0]).toBe("capture_lifecycle_pending:session-1");
  expect(JSON.parse(statement.params[1])).toEqual(marker);
  expect(statement.expectedRowsAffected).toBe(1);
});

test("rejects a different pending capture generation", async () => {
  mocks.executeTransaction.mockRejectedValueOnce(
    new Error("expected 1 affected row, got 0"),
  );

  await expect(saveCaptureLifecycleMarker(marker)).rejects.toThrow(
    "expected 1 affected row, got 0",
  );
});

test("loads a valid capture marker", async () => {
  mocks.execute.mockResolvedValue([{ value_json: JSON.stringify(marker) }]);

  await expect(loadCaptureLifecycleMarker("session-1")).resolves.toEqual(
    marker,
  );
});

test("preserves automatic capture provenance and its pre-recording audio state", async () => {
  const automaticMarker = {
    ...marker,
    automatic: true,
    preserveExistingAudio: false,
    initialTitle: "Standup",
  };
  mocks.execute.mockResolvedValue([
    { value_json: JSON.stringify(automaticMarker) },
  ]);
  await expect(loadCaptureLifecycleMarker("session-1")).resolves.toEqual(
    automaticMarker,
  );
});

test("loads the exact durable summary recovery mode", async () => {
  const summaryMarker = {
    ...marker,
    phase: "finalizing" as const,
    summaryMode: "regenerate" as const,
  };
  mocks.execute.mockResolvedValue([
    { value_json: JSON.stringify(summaryMarker) },
  ]);

  await expect(loadCaptureLifecycleMarker("session-1")).resolves.toEqual(
    summaryMarker,
  );
});

test("preserves the summary refresh requirement while batch repair is pending", async () => {
  const repairMarker = { ...marker, refreshSummaryAfterRepair: true };
  await saveCaptureLifecycleMarker(repairMarker);
  const statement = mocks.executeTransaction.mock.calls[0]?.[0]?.[0];
  mocks.execute.mockResolvedValue([{ value_json: statement.params[1] }]);

  await expect(loadCaptureLifecycleMarker("session-1")).resolves.toEqual(
    repairMarker,
  );
});

test.each([false, "true", 1, null])(
  "ignores an invalid summary refresh flag: %s",
  async (refreshSummaryAfterRepair) => {
    mocks.execute.mockResolvedValue([
      {
        value_json: JSON.stringify({ ...marker, refreshSummaryAfterRepair }),
      },
    ]);

    await expect(loadCaptureLifecycleMarker("session-1")).resolves.toEqual(
      marker,
    );
  },
);

test("ignores malformed or mismatched capture markers", async () => {
  mocks.execute.mockResolvedValue([
    {
      id: "capture_lifecycle_pending:session-1",
      value_json: JSON.stringify({ ...marker, sessionId: "other-session" }),
    },
    {
      id: "capture_lifecycle_pending:session-2",
      value_json: "{",
    },
  ]);

  await expect(loadCaptureLifecycleMarkers()).resolves.toEqual([]);
  expect(mocks.execute).toHaveBeenCalledWith(
    expect.stringContaining("WHERE id GLOB ?"),
    ["capture_lifecycle_pending:*"],
  );
});

test("clears only the matching capture generation", async () => {
  await clearCaptureLifecycleMarker("session-1", "transcript-1");

  const statement = mocks.executeTransaction.mock.calls[0]?.[0]?.[0];
  expect(statement.sql).toContain("json_extract");
  expect(statement.params).toEqual([
    "capture_lifecycle_pending:session-1",
    "transcript-1",
  ]);
  expect(statement.expectedRowsAffected).toBe(1);
});

test("rejects when a newer capture marker owns the session", async () => {
  mocks.executeTransaction.mockRejectedValueOnce(
    new Error("expected 1 affected row, got 0"),
  );

  await expect(
    clearCaptureLifecycleMarker("session-1", "stale-transcript"),
  ).rejects.toThrow("expected 1 affected row, got 0");
});

test.each([true, false])(
  "round-trips chunked capture and retention flags: %s",
  async (retainAudio) => {
    const saved = {
      ...marker,
      chunkedAudio: true,
      retainAudio,
      postStopBatch: retainAudio,
    };
    await saveCaptureLifecycleMarker(saved);
    const statement = mocks.executeTransaction.mock.calls[0]![0][0];
    mocks.execute.mockResolvedValue([{ value_json: statement.params[1] }]);
    await expect(loadCaptureLifecycleMarker(marker.sessionId)).resolves.toEqual(
      saved,
    );
  },
);

test.each(["true", 1, null])(
  "ignores invalid audio flags: %s",
  async (value) => {
    mocks.execute.mockResolvedValue([
      {
        value_json: JSON.stringify({
          ...marker,
          chunkedAudio: value,
          retainAudio: value,
          postStopBatch: value,
        }),
      },
    ]);
    await expect(loadCaptureLifecycleMarker(marker.sessionId)).resolves.toEqual(
      marker,
    );
  },
);
