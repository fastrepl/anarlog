import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  executeTransaction: vi.fn(
    (
      _statements: Array<{
        sql: string;
        params: unknown[];
        expectedRowsAffected: number;
      }>,
    ) => Promise.resolve([1, 1]),
  ),
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
}));

import { applySessionContentCorrections } from "./content-mutations";

describe("session content SQLite corrections", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("guards every summary and transcript update against stale content", async () => {
    await applySessionContentCorrections({
      sessionId: "session-1",
      summaries: [
        {
          id: "summary-1",
          currentContent: "old summary",
          currentContentFormat: "markdown",
          nextContent: '{"type":"doc"}',
        },
      ],
      transcripts: [
        {
          id: "transcript-1",
          currentWordsJson: '[{"text":"X"}]',
          currentMemo: "Speaker: X",
          nextWordsJson: '[{"text":"Y"}]',
          nextMemo: "Speaker: Y",
        },
      ],
    });

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements).toHaveLength(2);
    expect(statements[0]).toMatchObject({ expectedRowsAffected: 1 });
    expect(statements[0].sql).toContain("body = ?");
    expect(statements[0].sql).toContain("body_format = ?");
    expect(statements[1]).toMatchObject({ expectedRowsAffected: 1 });
    expect(statements[1].sql).toContain("words_json = ?");
    expect(statements[1].sql).toContain("memo = ?");
  });
});
