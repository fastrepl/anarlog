import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import {
  formatMeetingChatContext,
  loadMeetingChatRecords,
  MAX_MEETING_CHAT_RECORD_BYTES,
  MAX_MEETING_CHAT_RECORDS,
  persistMeetingChatRecords,
  useMeetingChatRecords,
} from "./meeting-chat-records";

const { executeMock, executeTransactionMock, useLiveQueryMock } = vi.hoisted(
  () => ({
    executeMock: vi.fn(),
    executeTransactionMock: vi.fn(),
    useLiveQueryMock: vi.fn(),
  }),
);

vi.mock("~/db", () => ({
  executeTransaction: executeTransactionMock,
  liveQueryClient: { execute: executeMock },
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

const platformLabels = [
  ["zoom", "Zoom"],
  ["googleMeet", "Google Meet"],
  ["microsoftTeams", "Microsoft Teams"],
  ["slack", "Slack"],
  ["discord", "Discord"],
  ["webex", "Webex"],
  ["unknown", "Meeting app"],
] as const;

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

  test("rejects an oversized record once without materializing or retrying it", async () => {
    const sourceSignature = "zoom\nnative\noversized";

    await expect(
      persistMeetingChatRecords({
        sessionId: "session-1",
        entries: [
          {
            message: {
              ...message,
              id: "oversized",
              text: "x".repeat(MAX_MEETING_CHAT_RECORD_BYTES * 4),
            },
            sourceSignature,
          },
        ],
      }),
    ).resolves.toEqual([sourceSignature]);

    expect(executeTransactionMock).not.toHaveBeenCalled();
  });

  test.each(platformLabels)(
    "persists %s records with the %s label",
    async (platform, label) => {
      await persistMeetingChatRecords({
        sessionId: "session-1",
        entries: [
          {
            message: { ...message, platform },
            sourceSignature: `${platform}\nnative\nax-chat-1`,
          },
        ],
      });

      const statement = executeTransactionMock.mock.calls[0]?.[0][0];
      expect(statement.params[1]).toBe(`${label} chat`);
    },
  );

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

  test("reads records from every meeting platform without relabeling them", () => {
    useLiveQueryMock.mockImplementation(
      ({ mapRows }: { mapRows: (rows: unknown[]) => unknown }) => ({
        data: mapRows([
          ...platformLabels.map(([platform], index) => ({
            id: `document-${index}`,
            body: JSON.stringify({
              ...message,
              id: `message-${index}`,
              platform,
              surface: platform === "unknown" ? "unknown" : "web",
            }),
            created_at: "2026-07-13T10:00:00.000Z",
          })),
          {
            id: "unsupported-platform",
            body: JSON.stringify({ ...message, platform: "other" }),
            created_at: "2026-07-13T10:00:00.000Z",
          },
        ]),
      }),
    );

    const { result } = renderHook(() => useMeetingChatRecords("session-1"));

    expect(result.current.map(({ platform }) => platform)).toEqual(
      platformLabels.map(([platform]) => platform),
    );
    expect(result.current[platformLabels.length - 1]?.surface).toBe("unknown");
  });

  test("loads ordered records imperatively and ignores malformed rows", async () => {
    executeMock.mockResolvedValue([
      {
        id: "document-1",
        body: JSON.stringify(message),
        created_at: "2026-07-13T10:00:00.000Z",
      },
      {
        id: "broken",
        body: "not json",
        created_at: "2026-07-13T10:00:01.000Z",
      },
    ]);

    await expect(loadMeetingChatRecords("session-1")).resolves.toEqual([
      {
        ...message,
        capturedAt: "2026-07-13T10:00:00.000Z",
      },
    ]);
    expect(executeMock).toHaveBeenCalledWith(
      expect.stringMatching(
        new RegExp(
          `length\\(CAST\\(body AS BLOB\\)\\) <= ${MAX_MEETING_CHAT_RECORD_BYTES}[\\s\\S]+LIMIT ${MAX_MEETING_CHAT_RECORDS}[\\s\\S]+ORDER BY sort_order, created_at, id`,
        ),
      ),
      ["session-1"],
    );
  });

  test("materializes only the newest bounded meeting-chat window", () => {
    useLiveQueryMock.mockImplementation(
      ({ mapRows }: { mapRows: (rows: unknown[]) => unknown }) => ({
        data: mapRows(
          Array.from({ length: MAX_MEETING_CHAT_RECORDS + 1 }, (_, index) => ({
            id: `document-${index}`,
            body: JSON.stringify({ ...message, id: `message-${index}` }),
            created_at: "2026-07-13T10:00:00.000Z",
          })),
        ),
      }),
    );

    const { result } = renderHook(() => useMeetingChatRecords("session-1"));

    expect(result.current).toHaveLength(MAX_MEETING_CHAT_RECORDS);
    expect(result.current[0]?.id).toBe("message-1");
    expect(result.current[result.current.length - 1]?.id).toBe(
      `message-${MAX_MEETING_CHAT_RECORDS}`,
    );
  });

  test("drops oversized meeting-chat documents before parsing", () => {
    useLiveQueryMock.mockImplementation(
      ({ mapRows }: { mapRows: (rows: unknown[]) => unknown }) => ({
        data: mapRows([
          {
            id: "oversized",
            body: "x".repeat(MAX_MEETING_CHAT_RECORD_BYTES + 1),
            created_at: "2026-07-13T10:00:00.000Z",
          },
        ]),
      }),
    );

    const { result } = renderHook(() => useMeetingChatRecords("session-1"));

    expect(result.current).toEqual([]);
  });

  test("formats a labeled meeting-chat context block", () => {
    expect(
      formatMeetingChatContext([
        {
          ...message,
          capturedAt: "2026-07-13T10:00:00.000Z",
        },
      ]),
    ).toBe(
      "## Meeting chat\n- Zoom · 10:42 AM · Ada · received\n  Review https://example.com/spec",
    );
    expect(formatMeetingChatContext([])).toBe("");
  });
});
