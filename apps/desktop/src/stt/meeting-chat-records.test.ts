import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import {
  persistMeetingChatRecords,
  useMeetingChatRecords,
} from "./meeting-chat-records";

const { executeTransactionMock, useLiveQueryMock } = vi.hoisted(() => ({
  executeTransactionMock: vi.fn(),
  useLiveQueryMock: vi.fn(),
}));

vi.mock("~/db", () => ({
  executeTransaction: executeTransactionMock,
  useLiveQuery: useLiveQueryMock,
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: async (_key: string, write: () => Promise<unknown>) =>
    write(),
}));

const message = {
  id: "ax-chat-1",
  platform: "zoom" as const,
  surface: "native" as const,
  sender: "Ada",
  timestamp: "10:42 AM",
  direction: "incoming" as const,
  text: "Review https://example.com/spec",
  links: ["https://example.com/spec"],
};

describe("meeting chat records", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-07-13T10:00:00.000Z"));
    executeTransactionMock.mockResolvedValue([1]);
    useLiveQueryMock.mockReturnValue({ data: [] });
  });

  test("persists idempotent meeting-chat documents without updating the memo", async () => {
    const request = {
      sessionId: "session-1",
      entries: [
        {
          message,
          sourceSignature: "zoom\nnative\nax-chat-1",
        },
      ],
    };

    await expect(persistMeetingChatRecords(request)).resolves.toEqual([
      "zoom\nnative\nax-chat-1",
    ]);
    await persistMeetingChatRecords(request);

    const firstStatement = executeTransactionMock.mock.calls[0]?.[0][0];
    const secondStatement = executeTransactionMock.mock.calls[1]?.[0][0];
    expect(firstStatement.sql).toContain("kind, title, body_format");
    expect(firstStatement.sql).toContain("'meeting_chat'");
    expect(firstStatement.sql).toContain("ON CONFLICT(id) DO NOTHING");
    expect(firstStatement.sql).not.toContain("UPDATE");
    expect(firstStatement.params[0]).toBe(secondStatement.params[0]);
    expect(JSON.parse(firstStatement.params[2])).toMatchObject({
      ...message,
      capturedAt: "2026-07-13T10:00:00.000Z",
    });
  });

  test("reads ordered valid records and ignores malformed rows", () => {
    useLiveQueryMock.mockImplementation(
      ({ mapRows }: { mapRows: (rows: unknown[]) => unknown }) => ({
        data: mapRows([
          {
            id: "document-1",
            body: JSON.stringify({
              ...message,
              links: ["https://example.com/spec", "javascript:alert(1)"],
              capturedAt: "2026-07-13T10:00:00.000Z",
            }),
            created_at: "2026-07-13T10:00:00.000Z",
          },
          {
            id: "broken",
            body: "not json",
            created_at: "2026-07-13T10:00:01.000Z",
          },
        ]),
      }),
    );

    const { result } = renderHook(() => useMeetingChatRecords("session-1"));

    expect(result.current).toEqual([
      {
        ...message,
        links: ["https://example.com/spec"],
        capturedAt: "2026-07-13T10:00:00.000Z",
      },
    ]);
    expect(useLiveQueryMock).toHaveBeenCalledWith(
      expect.objectContaining({
        params: ["session-1"],
        enabled: true,
      }),
    );
  });
});
