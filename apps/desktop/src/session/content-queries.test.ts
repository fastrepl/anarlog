import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ execute: vi.fn() }));

vi.mock("~/db", () => ({
  liveQueryClient: { execute: mocks.execute },
}));

import {
  loadActiveSessionIds,
  loadSessionContentSnapshot,
  parseAppliedTemplateSnapshot,
  resolveHeadingsUpdatedAt,
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
        owner_email: "user@example.com",
        title: "Planning",
        created_at: "2026-07-10T09:00:00.000Z",
        event_json: JSON.stringify({ title: "Weekly planning" }),
        event_id: "event-1",
        raw_note_id: "session-1",
        raw_template_id: "template-1",
        raw_metadata_json: JSON.stringify({
          appliedTemplate: {
            templateId: "template-1",
            sections: ["Raw note"],
          },
          headings: {
            key: "Raw note",
            updatedAt: "2026-07-10T09:00:00.000Z",
          },
        }),
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
            email: "alice@example.com",
            job_title: "Engineer",
          },
        ]),
      },
    ]);

    const snapshot = await loadSessionContentSnapshot("session-1");

    expect(snapshot).toMatchObject({
      sessionId: "session-1",
      ownerUserId: "user-1",
      ownerEmail: "user@example.com",
      title: "Planning",
      createdAt: "2026-07-10T09:00:00.000Z",
      event: { title: "Weekly planning" },
      eventId: "event-1",
      rawNoteId: "session-1",
      rawTemplateId: "template-1",
      rawHeadingsUpdatedAt: "",
      rawAppliedTemplate: {
        templateId: "template-1",
        sections: ["Raw note"],
      },
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
          speakerHintsJson: "[]",
          words: [expect.objectContaining({ id: "word-1", text: "Hello" })],
        },
      ],
      participants: [
        {
          humanId: "human-1",
          name: "Alice",
          email: "alice@example.com",
          jobTitle: "Engineer",
        },
      ],
    });
    expect(snapshot?.rawMarkdown).toContain("Raw note");
    expect(mocks.execute).toHaveBeenCalledWith(expect.any(String), [
      1,
      1,
      "session-1",
    ]);
    expect(mocks.execute.mock.calls[0][0]).toContain("self_human.email");
    expect(mocks.execute.mock.calls[0][0]).toContain(
      "COALESCE(NULLIF(human.email, ''), participant.email)",
    );
  });

  it("keeps transcript metadata without loading transcript words", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        id: "session-1",
        owner_user_id: "user-1",
        owner_email: null,
        title: "Planning",
        created_at: "2026-07-10T09:00:00.000Z",
        event_json: "null",
        source_apps_json: "[]",
        event_id: "",
        raw_note_id: "",
        raw_template_id: "",
        raw_body: "",
        raw_body_format: "markdown",
        enhanced_notes_json: "[]",
        transcripts_json: JSON.stringify([
          {
            id: "transcript-1",
            started_at_ms: 100,
            ended_at_ms: 200,
            memo: "Pre-meeting notes",
            words_json: "[]",
            speaker_hints_json: "[]",
          },
        ]),
        participants_json: "[]",
      },
    ]);

    const snapshot = await loadSessionContentSnapshot("session-1", {
      includeTranscriptWords: false,
    });

    expect(snapshot?.transcripts).toEqual([
      {
        id: "transcript-1",
        started_at: 100,
        ended_at: 200,
        memo: "Pre-meeting notes",
        wordsJson: "[]",
        speakerHintsJson: "[]",
        words: [],
        speaker_hints: [],
      },
    ]);
    expect(mocks.execute).toHaveBeenCalledWith(expect.any(String), [
      0,
      0,
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

  it("rejects malformed applied template snapshots", () => {
    expect(parseAppliedTemplateSnapshot("")).toBeNull();
    expect(parseAppliedTemplateSnapshot("not json")).toBeNull();
    expect(parseAppliedTemplateSnapshot("{}")).toBeNull();
    expect(
      parseAppliedTemplateSnapshot(JSON.stringify({ appliedTemplate: {} })),
    ).toBeNull();
    expect(
      parseAppliedTemplateSnapshot(
        JSON.stringify({
          appliedTemplate: { templateId: "template-1", sections: "nope" },
        }),
      ),
    ).toBeNull();
  });

  it("reads an empty headings time from malformed metadata", () => {
    const body = JSON.stringify({
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "Updates" }],
        },
      ],
    });
    expect(resolveHeadingsUpdatedAt("", body, "prosemirror_json")).toBe("");
    expect(resolveHeadingsUpdatedAt("{}", body, "prosemirror_json")).toBe("");
    expect(
      resolveHeadingsUpdatedAt(
        JSON.stringify({ headings: "nope" }),
        body,
        "prosemirror_json",
      ),
    ).toBe("");
    expect(
      resolveHeadingsUpdatedAt(
        JSON.stringify({
          headings: { key: "Updates", updatedAt: "2026-10-01T00:00:00Z" },
        }),
        body,
        "prosemirror_json",
      ),
    ).toBe("2026-10-01T00:00:00Z");
  });

  it("rejects a stored headings time when the body headings changed", () => {
    const metadata = JSON.stringify({
      headings: { key: "To-dos", updatedAt: "2026-10-01T00:00:00.000Z" },
    });
    const body = JSON.stringify({
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "To do" }],
        },
      ],
    });
    expect(resolveHeadingsUpdatedAt(metadata, body, "prosemirror_json")).toBe(
      "",
    );
    expect(
      resolveHeadingsUpdatedAt(metadata, "not json", "prosemirror_json"),
    ).toBe("");
  });

  it("normalizes title-only and legacy object snapshot sections", () => {
    expect(
      parseAppliedTemplateSnapshot(
        JSON.stringify({
          appliedTemplate: {
            templateId: "template-1",
            sections: [
              "  Updates  ",
              { title: "Action Items", description: "Follow-ups" },
              "",
              null,
              42,
            ],
          },
        }),
      ),
    ).toEqual({
      templateId: "template-1",
      sections: ["Updates", "Action Items"],
    });
  });
});
