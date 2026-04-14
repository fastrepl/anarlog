import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useDailySummarySnapshot } from "./api";

const { executeMock, subscribeMock } = vi.hoisted(() => ({
  executeMock: vi.fn(),
  subscribeMock: vi.fn(),
}));

vi.mock("@hypr/plugin-db", () => ({
  execute: executeMock,
  executeProxy: vi.fn().mockResolvedValue({ rows: [] }),
  subscribe: subscribeMock,
}));

type SubscribeOptions<T> = {
  onData: (rows: T[]) => void;
  onError?: (message: string) => void;
};

describe("useDailySummarySnapshot", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("maps subscribed SQLite rows into a daily summary snapshot", async () => {
    const subscriptions: Array<SubscribeOptions<any>> = [];

    subscribeMock.mockImplementation(async (_sql, _params, options) => {
      subscriptions.push(options);
      return vi.fn();
    });

    const { result } = renderHook(() =>
      useDailySummarySnapshot({
        date: "2026-04-13",
        startMs: 100,
        endMs: 200,
      }),
    );

    await waitFor(() => {
      expect(subscribeMock).toHaveBeenCalledTimes(3);
    });

    act(() => {
      subscriptions[0]?.onData([
        {
          capturedAtMs: 125,
          observationId: "obs-1",
          screenshotId: "ss-1",
          screenshotKind: "settled",
          appName: "Cursor",
          windowTitle: "apps/desktop/src/daily-summary/tab-content.tsx",
          summary: "Refactoring the daily summary tab.",
        },
      ]);
      subscriptions[1]?.onData([
        {
          observationCount: 3,
          screenshotCount: 4,
          analysisCount: 1,
          uniqueAppCount: 2,
          firstObservationAtMs: 101,
          lastObservationAtMs: 199,
          topAppsJson:
            '[{"appName":"Cursor","count":2},{"appName":"Safari","count":1}]',
          eventCursorMs: 199,
          analysisCursorMs: 125,
        },
      ]);
      subscriptions[2]?.onData([
        {
          id: "daily-summary-2026-04-13",
          date: "2026-04-13",
          content: "# Summary",
          timeline_json:
            '[{"time":"10:00","summary":"Worked on the daily summary view."}]',
          topics_json:
            '[{"title":"Desktop","summary":"Daily summary moved to SQLite."}]',
          status: "ready",
          source_cursor_ms: 199,
          source_fingerprint: "ignored-by-hook",
          generated_at: "2026-04-13T10:30:00Z",
          generation_error: "",
          updated_at: "2026-04-13T10:30:00Z",
        },
      ]);
    });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
      expect(result.current.data).toEqual({
        stats: {
          observationCount: 3,
          screenshotCount: 4,
          analysisCount: 1,
          uniqueAppCount: 2,
          firstObservationAtMs: 101,
          lastObservationAtMs: 199,
          topApps: [
            { appName: "Cursor", count: 2 },
            { appName: "Safari", count: 1 },
          ],
        },
        analyses: [
          {
            capturedAtMs: 125,
            observationId: "obs-1",
            screenshotId: "ss-1",
            screenshotKind: "settled",
            appName: "Cursor",
            windowTitle: "apps/desktop/src/daily-summary/tab-content.tsx",
            summary: "Refactoring the daily summary tab.",
          },
        ],
        summary: {
          id: "daily-summary-2026-04-13",
          date: "2026-04-13",
          content: "# Summary",
          timeline: [
            {
              time: "10:00",
              summary: "Worked on the daily summary view.",
            },
          ],
          topics: [
            {
              title: "Desktop",
              summary: "Daily summary moved to SQLite.",
            },
          ],
          status: "ready",
          sourceCursorMs: 199,
          sourceFingerprint: "ignored-by-hook",
          generatedAt: "2026-04-13T10:30:00Z",
          generationError: "",
          updatedAt: "2026-04-13T10:30:00Z",
        },
        sourceCursorMs: 199,
        sourceFingerprint: "observations:3|screenshots:4|analyses:1|cursor:199",
      });
    });
  });

  it("updates the stored summary when the summary subscription changes", async () => {
    const subscriptions: Array<SubscribeOptions<any>> = [];

    subscribeMock.mockImplementation(async (_sql, _params, options) => {
      subscriptions.push(options);
      return vi.fn();
    });

    const { result } = renderHook(() =>
      useDailySummarySnapshot({
        date: "2026-04-13",
        startMs: 100,
        endMs: 200,
      }),
    );

    await waitFor(() => {
      expect(subscribeMock).toHaveBeenCalledTimes(3);
    });

    act(() => {
      subscriptions[0]?.onData([]);
      subscriptions[1]?.onData([
        {
          observationCount: 0,
          screenshotCount: 0,
          analysisCount: 0,
          uniqueAppCount: 0,
          firstObservationAtMs: null,
          lastObservationAtMs: null,
          topAppsJson: "[]",
          eventCursorMs: 0,
          analysisCursorMs: 0,
        },
      ]);
      subscriptions[2]?.onData([]);
    });

    await waitFor(() => {
      expect(result.current.data?.summary).toBeNull();
    });

    act(() => {
      subscriptions[2]?.onData([
        {
          id: "daily-summary-2026-04-13",
          date: "2026-04-13",
          content: "# New Summary",
          timeline_json: "[]",
          topics_json: "[]",
          status: "ready",
          source_cursor_ms: 0,
          source_fingerprint: "",
          generated_at: "2026-04-13T10:45:00Z",
          generation_error: "",
          updated_at: "2026-04-13T10:45:00Z",
        },
      ]);
    });

    await waitFor(() => {
      expect(result.current.data?.summary?.content).toBe("# New Summary");
    });
  });
});
