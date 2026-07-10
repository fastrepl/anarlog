import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ execute: vi.fn() }));

vi.mock("~/db", () => ({
  liveQueryClient: { execute: mocks.execute },
}));

import {
  loadActiveSessionIds,
  loadSessionContentSnapshot,
} from "./content-queries";

describe("session content SQLite snapshots", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("maps one canonical session content snapshot", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        id: "session-1",
        owner_user_id: "user-1",
        title: "Planning",
        created_at: "2026-07-10T09:00:00.000Z",
        event_json: JSON.stringify({ title: "Weekly planning" }),
        event_id: "event-1",
        raw_note_id: "session-1",
        raw_body: JSON.stringify({
          type: "doc",
          content: [
            {
              type: "paragraph",
              content: [{ type: "text", text: "Raw note" }],
            },
          ],
        }),
        raw_body_format: "prosemirror_json",
        enhanced_notes_json: JSON.stringify([
          {
            id: "summary-2",
            title: "Second",
            body: "Second summary",
            body_format: "markdown",
            template_id: "template-2",
            sort_order: 2,
          },
          {
            id: "summary-1",
            title: "First",
            body: "First summary",
            body_format: "markdown",
            template_id: "template-1",
            sort_order: 1,
          },
        ]),
        transcripts_json: JSON.stringify([
          {
            id: "transcript-1",
            started_at_ms: 100,
            ended_at_ms: 200,
            memo: "",
            words_json: JSON.stringify([
              {
                id: "word-1",
                text: "Hello",
                start_ms: 0,
                end_ms: 100,
                channel: 0,
              },
            ]),
            speaker_hints_json: "[]",
          },
        ]),
        participants_json: JSON.stringify([
          {
            human_id: "human-1",
            name: "Alice",
            job_title: "Engineer",
          },
        ]),
      },
    ]);

    const snapshot = await loadSessionContentSnapshot("session-1");

    expect(snapshot).toMatchObject({
      sessionId: "session-1",
      ownerUserId: "user-1",
      title: "Planning",
      createdAt: "2026-07-10T09:00:00.000Z",
      event: { title: "Weekly planning" },
      eventId: "event-1",
      rawNoteId: "session-1",
      rawContentFormat: "prosemirror_json",
      enhancedNotes: [
        { id: "summary-1", markdown: "First summary", position: 1 },
        { id: "summary-2", markdown: "Second summary", position: 2 },
      ],
      transcripts: [
        {
          id: "transcript-1",
          started_at: 100,
          ended_at: 200,
          words: [expect.objectContaining({ id: "word-1", text: "Hello" })],
        },
      ],
      participants: [
        { humanId: "human-1", name: "Alice", jobTitle: "Engineer" },
      ],
    });
    expect(snapshot?.rawMarkdown).toContain("Raw note");
    expect(mocks.execute).toHaveBeenCalledWith(expect.any(String), [
      "session-1",
    ]);
  });

  it("lists only active SQLite session ids", async () => {
    mocks.execute.mockResolvedValueOnce([
      { id: "session-2" },
      { id: "session-1" },
    ]);

    await expect(loadActiveSessionIds()).resolves.toEqual([
      "session-2",
      "session-1",
    ]);
    expect(mocks.execute.mock.calls[0][0]).toContain("deleted_at IS NULL");
  });
});
