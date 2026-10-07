import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { enhanceTransform } from "./enhance-transform";

import { assertCanonicalTemplateSections } from "~/templates/codec";

const mocks = vi.hoisted(() => ({
  collectEnhanceImageContext: vi.fn(),
  getTemplateById: vi.fn(),
  formatMeetingChatContext: vi.fn(),
  loadMeetingChatRecords: vi.fn(),
  loadSessionContentSnapshot: vi.fn(),
  renderSessionTranscript: vi.fn(),
  dominantLanguage: vi.fn(),
  summaryLengthPolicy: vi.fn(),
}));

vi.mock("./enhance-images", () => ({
  collectEnhanceImageContext: mocks.collectEnhanceImageContext,
}));

vi.mock("~/templates/queries", () => ({
  getTemplateById: mocks.getTemplateById,
}));

vi.mock("~/session/content-queries", () => ({
  loadSessionContentSnapshot: mocks.loadSessionContentSnapshot,
}));

vi.mock("@anlg/plugin-transcription", () => ({
  commands: { renderSessionTranscript: mocks.renderSessionTranscript },
}));

vi.mock("@anlg/plugin-template", () => ({
  commands: {
    dominantLanguage: mocks.dominantLanguage,
    summaryLengthPolicy: mocks.summaryLengthPolicy,
  },
}));

vi.mock("~/stt/meeting-chat-records", () => ({
  formatMeetingChatContext: mocks.formatMeetingChatContext,
  loadMeetingChatRecords: mocks.loadMeetingChatRecords,
}));

function createSnapshot() {
  return {
    sessionId: "session-1",
    ownerUserId: "user-1",
    title: "Weekly Review",
    createdAt: "2026-07-10T00:00:00.000Z",
    event: null,
    sourceApps: [],
    eventId: null,
    rawNoteId: "session-1",
    rawTemplateId: "",
    rawUpdatedAt: "",
    rawHeadingsUpdatedAt: "",
    rawAppliedTemplate: null,
    rawContent: "![post](asset://localhost/post.png)",
    rawContentFormat: "markdown",
    rawMarkdown: "![post](asset://localhost/post.png)",
    enhancedNotes: [],
    transcripts: [
      {
        id: "transcript-1",
        started_at: 100,
        ended_at: 200,
        memo: "![pre](asset://localhost/pre.png)",
        wordsJson: "[]",
        words: [],
        speaker_hints: [],
      },
    ],
    participants: [{ humanId: "human-1", name: "Alice", jobTitle: "Engineer" }],
  };
}

const settingsValues = { ai_language: "en" } as const;

function docWithHeadings(...titles: string[]) {
  return JSON.stringify({
    type: "doc",
    content: titles.flatMap((title) => [
      {
        type: "heading",
        attrs: { level: 2 },
        content: [{ type: "text", text: title }],
      },
      { type: "paragraph" },
    ]),
  });
}

async function runTransform(
  snapshotOverrides: Record<string, unknown>,
  template: unknown,
) {
  mocks.loadSessionContentSnapshot.mockResolvedValue({
    ...createSnapshot(),
    ...snapshotOverrides,
  });
  mocks.getTemplateById.mockResolvedValue(template);
  return enhanceTransform.transformArgs(
    {
      sessionId: "session-1",
      enhancedNoteId: "note-1",
      templateId: "template-1",
    },
    settingsValues,
  );
}

describe("enhanceTransform.transformArgs", () => {
  let consoleError: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.collectEnhanceImageContext.mockResolvedValue([]);
    mocks.getTemplateById.mockResolvedValue(null);
    mocks.formatMeetingChatContext.mockReturnValue("");
    mocks.loadMeetingChatRecords.mockResolvedValue([]);
    mocks.loadSessionContentSnapshot.mockResolvedValue(createSnapshot());
    mocks.renderSessionTranscript.mockResolvedValue({
      status: "ok",
      data: null,
    });
    mocks.summaryLengthPolicy.mockResolvedValue({
      status: "ok",
      data: null,
    });
    mocks.dominantLanguage.mockResolvedValue({
      status: "ok",
      data: null,
    });
    consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    consoleError.mockRestore();
  });

  it("uses the selected template when it can be loaded", async () => {
    mocks.getTemplateById.mockResolvedValue({
      title: "Standup",
      description: "Daily sync",
      sections: [{ title: "Updates", description: null }],
    });

    const result = await enhanceTransform.transformArgs(
      {
        sessionId: "session-1",
        enhancedNoteId: "note-1",
        templateId: "template-1",
      },
      settingsValues,
    );

    expect(result.template).toEqual({
      title: "Standup",
      description: "Daily sync",
      sections: [{ title: "Updates", description: null }],
    });
    expect(result.participants).toEqual([
      { name: "Alice", jobTitle: "Engineer" },
    ]);
  });

  it("includes the detected meeting platform in generated context", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue({
      ...createSnapshot(),
      sourceApps: [
        {
          app: "chrome",
          name: "Google Chrome",
          platform: "Google Meet",
        },
      ],
    });

    const result = await enhanceTransform.transformArgs(
      {
        sessionId: "session-1",
        enhancedNoteId: "note-1",
      },
      settingsValues,
    );

    expect(result.postMeetingMemo).toContain("Meeting platform: Google Meet");
  });

  it("keeps the pre-meeting memo when transcript words are omitted", async () => {
    const result = await enhanceTransform.transformArgs(
      {
        sessionId: "session-1",
        enhancedNoteId: "note-1",
      },
      settingsValues,
    );

    expect(result.preMeetingMemo).toBe("![pre](asset://localhost/pre.png)");
  });

  it("uses the edited memo headings for its applied template", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawContent: JSON.stringify({
          type: "doc",
          content: [
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Updates" }],
            },
            { type: "paragraph" },
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Next Steps" }],
            },
          ],
        }),
        rawContentFormat: "prosemirror_json",
        rawMarkdown: "## Updates\n\n## Next Steps",
      },
      {
        title: "1:1 Meeting",
        description: "Weekly conversation",
        sections: [
          { title: "Updates", description: "Recent changes" },
          { title: "Action Items", description: "Follow-ups" },
        ],
      },
    );

    expect(result.template).toEqual({
      title: "1:1 Meeting",
      description: "Weekly conversation",
      sections: [
        { title: "Updates", description: "Recent changes" },
        { title: "Next Steps", description: "Follow-ups" },
      ],
    });
  });

  it("keeps template sections added after the memo was written", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-01T00:00:00.000Z",
        rawContent: JSON.stringify({
          type: "doc",
          content: [
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Updates" }],
            },
            { type: "paragraph" },
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Action Items" }],
            },
          ],
        }),
        rawContentFormat: "prosemirror_json",
        rawMarkdown: "## Updates\n\n## Action Items",
      },
      {
        title: "1:1 Meeting",
        description: "Weekly conversation",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [
          { title: "TL;DR", description: "One-sentence overview" },
          { title: "Updates", description: "Recent changes" },
          { title: "Action Items", description: "Follow-ups" },
        ],
      },
    );

    expect(result.template?.sections.map((section) => section.title)).toEqual([
      "TL;DR",
      "Updates",
      "Action Items",
    ]);
  });

  it("keeps added template sections when counts match but positions moved", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-01T00:00:00.000Z",
        rawContent: JSON.stringify({
          type: "doc",
          content: [
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Updates" }],
            },
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Action Items" }],
            },
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Custom" }],
            },
          ],
        }),
        rawContentFormat: "prosemirror_json",
        rawMarkdown: "## Updates\n\n## Action Items\n\n## Custom",
      },
      {
        title: "1:1 Meeting",
        description: "Weekly conversation",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [
          { title: "TL;DR", description: "One-sentence overview" },
          { title: "Updates", description: "Recent changes" },
          { title: "Action Items", description: "Follow-ups" },
        ],
      },
    );

    expect(result.template?.sections.map((section) => section.title)).toEqual([
      "TL;DR",
      "Updates",
      "Action Items",
      "Custom",
    ]);
  });

  it("produces render-valid sections when memo and template diverge", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-01T00:00:00.000Z",
        rawContent: JSON.stringify({
          type: "doc",
          content: [
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Updates" }],
            },
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Next Steps" }],
            },
          ],
        }),
        rawContentFormat: "prosemirror_json",
        rawMarkdown: "## Updates\n\n## Next Steps",
      },
      {
        title: "1:1 Meeting",
        description: "Weekly conversation",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [
          { title: "TL;DR", description: "One-sentence overview" },
          { title: "Updates", description: "Recent changes" },
          { title: "Action Items", description: "Follow-ups" },
        ],
      },
    );

    expect(result.template?.sections.map((section) => section.title)).toEqual([
      "TL;DR",
      "Updates",
      "Action Items",
      "Next Steps",
    ]);
    for (const section of result.template?.sections ?? []) {
      expect(typeof section.description).toBe("string");
    }
    expect(() =>
      assertCanonicalTemplateSections(
        result.template?.sections,
        "enhance render template.sections",
      ),
    ).not.toThrow();
  });

  it("prefers the live template when it was saved after the memo", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-01T00:00:00.000Z",
        rawContent: JSON.stringify({
          type: "doc",
          content: [
            "Custom",
            "Updates & Discussions",
            "Done",
            "In Progress",
            "To do",
          ].flatMap((title) => [
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: title }],
            },
            { type: "paragraph" },
          ]),
        }),
        rawContentFormat: "prosemirror_json",
      },
      {
        title: "Scrum",
        description: "Quick syncs",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [
          { title: "TL;DR", description: "One-sentence overview" },
          { title: "Updates & Discussions", description: "Updates" },
          { title: "Done", description: "Done" },
          { title: "In Progress", description: "In progress" },
          { title: "To do", description: "To do" },
        ],
      },
    );

    expect(result.template?.sections.map((section) => section.title)).toEqual([
      "TL;DR",
      "Updates & Discussions",
      "Done",
      "In Progress",
      "To do",
      "Custom",
    ]);
  });

  it("keeps memo renames when the memo was written after the template", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-06T00:00:00.000Z",
        rawContent: JSON.stringify({
          type: "doc",
          content: [
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Updates" }],
            },
            { type: "paragraph" },
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Next Steps" }],
            },
          ],
        }),
        rawContentFormat: "prosemirror_json",
        rawMarkdown: "## Updates\n\n## Next Steps",
      },
      {
        title: "1:1 Meeting",
        description: "Weekly conversation",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [
          { title: "Updates", description: "Recent changes" },
          { title: "Action Items", description: "Follow-ups" },
        ],
      },
    );

    expect(result.template?.sections).toEqual([
      { title: "Updates", description: "Recent changes" },
      { title: "Next Steps", description: "Follow-ups" },
    ]);
  });

  it("follows template reordering when the template was saved after the memo", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-01T00:00:00.000Z",
        rawContent: JSON.stringify({
          type: "doc",
          content: [
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Action Items" }],
            },
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Updates" }],
            },
          ],
        }),
        rawContentFormat: "prosemirror_json",
        rawMarkdown: "## Action Items\n\n## Updates",
      },
      {
        title: "1:1 Meeting",
        description: "Weekly conversation",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [
          { title: "Updates", description: "Recent changes" },
          { title: "Action Items", description: "Follow-ups" },
        ],
      },
    );

    expect(result.template?.sections.map((section) => section.title)).toEqual([
      "Updates",
      "Action Items",
    ]);
  });

  it("keeps added template sections when the memo was edited after them", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-06T00:00:00.000Z",
        rawAppliedTemplate: {
          templateId: "template-1",
          sections: ["Updates & Discussions", "Done", "In Progress", "To do"],
        },
        rawContent: JSON.stringify({
          type: "doc",
          content: [
            "Custom",
            "Updates & Discussions",
            "Done",
            "In Progress",
            "To do",
          ].flatMap((title) => [
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: title }],
            },
            { type: "paragraph" },
          ]),
        }),
        rawContentFormat: "prosemirror_json",
      },
      {
        title: "Scrum",
        description: "Quick syncs",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [
          { title: "TL;DR", description: "One-sentence overview" },
          { title: "Updates & Discussions", description: "Updates" },
          { title: "Done", description: "Done" },
          { title: "In Progress", description: "In progress" },
          { title: "To do", description: "To do" },
        ],
      },
    );

    expect(result.template?.sections.map((section) => section.title)).toEqual([
      "TL;DR",
      "Updates & Discussions",
      "Done",
      "In Progress",
      "To do",
      "Custom",
    ]);
  });

  it("keeps memo renames when the unchanged template was saved after them", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-04T00:00:00.000Z",
        rawAppliedTemplate: {
          templateId: "template-1",
          sections: ["Updates", "Action Items"],
        },
        rawContent: JSON.stringify({
          type: "doc",
          content: [
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Updates" }],
            },
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Next Steps" }],
            },
          ],
        }),
        rawContentFormat: "prosemirror_json",
        rawMarkdown: "## Updates\n\n## Next Steps",
      },
      {
        title: "1:1 Meeting",
        description: "Weekly conversation",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [
          { title: "Updates", description: "Recent changes" },
          { title: "Action Items", description: "Follow-ups" },
        ],
      },
    );

    expect(result.template?.sections).toEqual([
      { title: "Updates", description: "Recent changes" },
      { title: "Next Steps", description: "Follow-ups" },
    ]);
  });

  it("keeps guidance for a rename below a memo-added heading", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-06T00:00:00.000Z",
        rawAppliedTemplate: {
          templateId: "template-1",
          sections: ["Updates", "Action Items"],
        },
        rawContent: JSON.stringify({
          type: "doc",
          content: ["Custom", "Updates", "Next Steps"].flatMap((title) => [
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: title }],
            },
            { type: "paragraph" },
          ]),
        }),
        rawContentFormat: "prosemirror_json",
      },
      {
        title: "1:1 Meeting",
        description: "Weekly conversation",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [
          { title: "Updates", description: "Recent changes" },
          { title: "Action Items", description: "Follow-ups" },
        ],
      },
    );

    expect(result.template?.sections).toEqual([
      { title: "Updates", description: "Recent changes" },
      { title: "Next Steps", description: "Follow-ups" },
      { title: "Custom", description: "" },
    ]);
  });

  it("pairs rewritten headings with applied sections by relative position", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-06T00:00:00.000Z",
        rawAppliedTemplate: {
          templateId: "template-1",
          sections: ["Alpha", "Beta"],
        },
        rawContent: JSON.stringify({
          type: "doc",
          content: ["One", "Two"].flatMap((title) => [
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: title }],
            },
            { type: "paragraph" },
          ]),
        }),
        rawContentFormat: "prosemirror_json",
      },
      {
        title: "Pair",
        description: "Rewrite",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [
          { title: "Alpha", description: "First" },
          { title: "Beta", description: "Second" },
        ],
      },
    );

    expect(result.template?.sections).toEqual([
      { title: "One", description: "First" },
      { title: "Two", description: "Second" },
    ]);
  });

  it("keeps both headings when template and memo change the same section", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-06T00:00:00.000Z",
        rawAppliedTemplate: {
          templateId: "template-1",
          sections: ["Updates", "Action Items"],
        },
        rawContent: JSON.stringify({
          type: "doc",
          content: ["Updates", "Next Steps"].flatMap((title) => [
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: title }],
            },
            { type: "paragraph" },
          ]),
        }),
        rawContentFormat: "prosemirror_json",
      },
      {
        title: "1:1 Meeting",
        description: "Weekly conversation",
        updatedAt: "2026-10-07T00:00:00Z",
        sections: [
          { title: "Updates", description: "Recent changes" },
          { title: "Follow-ups List", description: "Tracked follow-ups" },
        ],
      },
    );

    expect(result.template?.sections).toEqual([
      { title: "Updates", description: "Recent changes" },
      { title: "Follow-ups List", description: "Tracked follow-ups" },
      { title: "Next Steps", description: "" },
    ]);
  });

  it("keeps template order when the memo reordered and the template grew", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-06T00:00:00.000Z",
        rawAppliedTemplate: {
          templateId: "template-1",
          sections: ["Updates", "Action Items"],
        },
        rawContent: JSON.stringify({
          type: "doc",
          content: ["Action Items", "Updates"].flatMap((title) => [
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: title }],
            },
            { type: "paragraph" },
          ]),
        }),
        rawContentFormat: "prosemirror_json",
      },
      {
        title: "1:1 Meeting",
        description: "Weekly conversation",
        updatedAt: "2026-10-07T00:00:00Z",
        sections: [
          { title: "TL;DR", description: "One-sentence overview" },
          { title: "Updates", description: "Recent changes" },
          { title: "Action Items", description: "Follow-ups" },
        ],
      },
    );

    expect(result.template?.sections.map((section) => section.title)).toEqual([
      "TL;DR",
      "Updates",
      "Action Items",
    ]);
  });

  it("omits sections the memo deleted after they were applied", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-06T00:00:00.000Z",
        rawAppliedTemplate: {
          templateId: "template-1",
          sections: ["Updates", "Action Items"],
        },
        rawContent: JSON.stringify({
          type: "doc",
          content: [
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Updates" }],
            },
          ],
        }),
        rawContentFormat: "prosemirror_json",
        rawMarkdown: "## Updates",
      },
      {
        title: "1:1 Meeting",
        description: "Weekly conversation",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [
          { title: "Updates", description: "Recent changes" },
          { title: "Action Items", description: "Follow-ups" },
        ],
      },
    );

    expect(result.template?.sections.map((section) => section.title)).toEqual([
      "Updates",
    ]);
  });

  it("omits snapshot-less memo sections deleted after the template was saved", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-06T00:00:00.000Z",
        rawHeadingsUpdatedAt: "2026-10-06T00:00:00.000Z",
        rawContent: docWithHeadings("Updates"),
        rawContentFormat: "prosemirror_json",
        rawMarkdown: "## Updates",
      },
      {
        title: "1:1 Meeting",
        description: "Weekly conversation",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [
          { title: "Updates", description: "Recent changes" },
          { title: "Action Items", description: "Follow-ups" },
        ],
      },
    );

    expect(result.template?.sections.map((section) => section.title)).toEqual([
      "Updates",
    ]);
  });

  it("follows template reordering when the memo matches the snapshot", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-01T00:00:00.000Z",
        rawAppliedTemplate: {
          templateId: "template-1",
          sections: ["Updates", "Action Items"],
        },
        rawContent: JSON.stringify({
          type: "doc",
          content: [
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Updates" }],
            },
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Action Items" }],
            },
          ],
        }),
        rawContentFormat: "prosemirror_json",
        rawMarkdown: "## Updates\n\n## Action Items",
      },
      {
        title: "1:1 Meeting",
        description: "Weekly conversation",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [
          { title: "Action Items", description: "Follow-ups" },
          { title: "Updates", description: "Recent changes" },
        ],
      },
    );

    expect(result.template?.sections.map((section) => section.title)).toEqual([
      "Action Items",
      "Updates",
    ]);
  });

  it("follows the newer template reorder when both sides reordered the snapshot", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-06T00:00:00.000Z",
        rawAppliedTemplate: {
          templateId: "template-1",
          sections: ["Alpha", "Beta", "Gamma"],
        },
        rawContent: docWithHeadings("Beta", "Alpha", "Gamma"),
        rawContentFormat: "prosemirror_json",
      },
      {
        title: "Trio",
        description: "Three sections",
        updatedAt: "2026-10-07T00:00:00Z",
        sections: [
          { title: "Gamma", description: "Third" },
          { title: "Alpha", description: "First" },
          { title: "Beta", description: "Second" },
        ],
      },
    );

    expect(result.template?.sections).toEqual([
      { title: "Gamma", description: "Third" },
      { title: "Alpha", description: "First" },
      { title: "Beta", description: "Second" },
    ]);
  });

  it("drops a stale memo heading renamed in the template saved after the memo", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-01T00:00:00.000Z",
        rawContent: JSON.stringify({
          type: "doc",
          content: [
            "Previous Sprint & Updates",
            "Next Sprint Goal",
            "To-dos",
          ].flatMap((title) => [
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: title }],
            },
            { type: "paragraph" },
          ]),
        }),
        rawContentFormat: "prosemirror_json",
      },
      {
        title: "Sprint",
        description: "Sprint review",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [
          { title: "TL;DR", description: "One-sentence overview" },
          { title: "Previous Sprint & Updates", description: "Updates" },
          { title: "Next Sprint Goal", description: "Goal" },
          { title: "To do", description: "To do" },
        ],
      },
    );

    expect(result.template?.sections.map((section) => section.title)).toEqual([
      "TL;DR",
      "Previous Sprint & Updates",
      "Next Sprint Goal",
      "To do",
    ]);
  });

  it("keeps memo headings when the applied template is missing", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawAppliedTemplate: {
          templateId: "template-1",
          sections: ["Updates", "Action Items"],
        },
        rawContent: JSON.stringify({
          type: "doc",
          content: [
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Updates" }],
            },
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "Action Items" }],
            },
          ],
        }),
        rawContentFormat: "prosemirror_json",
        rawMarkdown: "## Updates\n\n## Action Items",
      },
      null,
    );

    expect(result.template).toEqual({
      title: "Meeting memo",
      description: null,
      sections: [
        { title: "Updates", description: "" },
        { title: "Action Items", description: "" },
      ],
    });
  });

  it("honors an emptied live template instead of restoring old headings", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-01T00:00:00.000Z",
        rawAppliedTemplate: {
          templateId: "template-1",
          sections: ["Updates"],
        },
        rawContent: docWithHeadings("Updates"),
        rawContentFormat: "prosemirror_json",
        rawMarkdown: "## Updates",
      },
      {
        title: "1:1 Meeting",
        description: "Weekly conversation",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [],
      },
    );

    expect(result.template?.sections).toEqual([]);
  });

  it("keeps short custom headings that resemble template titles", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-01T00:00:00.000Z",
        rawContent: docWithHeadings("Updates", "Tasks", "Risks"),
        rawContentFormat: "prosemirror_json",
      },
      {
        title: "1:1 Meeting",
        description: "Weekly conversation",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [
          { title: "Updates", description: "Recent changes" },
          { title: "Tasks", description: "Open tasks" },
        ],
      },
    );

    expect(result.template?.sections.map((section) => section.title)).toEqual([
      "Updates",
      "Tasks",
      "Risks",
    ]);
  });

  it("keeps custom headings resembling matched template titles", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-01T00:00:00.000Z",
        rawContent: docWithHeadings("Development", "Deployment"),
        rawContentFormat: "prosemirror_json",
      },
      {
        title: "Release",
        description: "Ship checklist",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [{ title: "Development", description: "Dev updates" }],
      },
    );

    expect(result.template?.sections.map((section) => section.title)).toEqual([
      "Development",
      "Deployment",
    ]);
  });

  it("keeps a custom heading that swaps a word in an added template title", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-01T00:00:00.000Z",
        rawContent: docWithHeadings("Updates", "Hiring"),
        rawContentFormat: "prosemirror_json",
      },
      {
        title: "1:1 Meeting",
        description: "Weekly conversation",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [
          { title: "Updates", description: "Recent changes" },
          { title: "Firing", description: "Offboarding" },
        ],
      },
    );

    expect(result.template?.sections.map((section) => section.title)).toEqual([
      "Updates",
      "Firing",
      "Hiring",
    ]);
  });

  it("drops a Korean memo heading renamed in the newer template", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-01T00:00:00.000Z",
        rawContent: docWithHeadings("업데이트", "할일 목록"),
        rawContentFormat: "prosemirror_json",
      },
      {
        title: "주간 회의",
        description: "주간 대화",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [
          { title: "업데이트", description: "최근 변경" },
          { title: "할 일 목록", description: "할 일" },
        ],
      },
    );

    expect(result.template?.sections.map((section) => section.title)).toEqual([
      "업데이트",
      "할 일 목록",
    ]);
  });

  it("drops stale headings when a paragraph edit postdates the template", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-06T00:00:00.000Z",
        rawHeadingsUpdatedAt: "2026-10-01T00:00:00.000Z",
        rawContent: docWithHeadings(
          "Previous Sprint & Updates",
          "Next Sprint Goal",
          "To-dos",
        ),
        rawContentFormat: "prosemirror_json",
      },
      {
        title: "Sprint",
        description: "Sprint review",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [
          { title: "TL;DR", description: "One-sentence overview" },
          { title: "Previous Sprint & Updates", description: "Updates" },
          { title: "Next Sprint Goal", description: "Goal" },
          { title: "To do", description: "To do" },
        ],
      },
    );

    expect(result.template?.sections.map((section) => section.title)).toEqual([
      "TL;DR",
      "Previous Sprint & Updates",
      "Next Sprint Goal",
      "To do",
    ]);
  });

  it("keeps template additions when only memo paragraphs changed after them", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-06T00:00:00.000Z",
        rawHeadingsUpdatedAt: "2026-10-01T00:00:00.000Z",
        rawContent: docWithHeadings("Custom", "Updates", "Done"),
        rawContentFormat: "prosemirror_json",
      },
      {
        title: "Scrum",
        description: "Quick syncs",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [
          { title: "TL;DR", description: "One-sentence overview" },
          { title: "Updates", description: "Recent changes" },
          { title: "Done", description: "Done" },
        ],
      },
    );

    expect(result.template?.sections.map((section) => section.title)).toEqual([
      "TL;DR",
      "Updates",
      "Done",
      "Custom",
    ]);
  });

  it("keeps memo renames newer than the template by headings time", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawUpdatedAt: "2026-10-06T00:00:00.000Z",
        rawHeadingsUpdatedAt: "2026-10-06T00:00:00.000Z",
        rawContent: docWithHeadings("Updates", "Next Steps"),
        rawContentFormat: "prosemirror_json",
        rawMarkdown: "## Updates\n\n## Next Steps",
      },
      {
        title: "1:1 Meeting",
        description: "Weekly conversation",
        updatedAt: "2026-10-05T00:00:00Z",
        sections: [
          { title: "Updates", description: "Recent changes" },
          { title: "Action Items", description: "Follow-ups" },
        ],
      },
    );

    expect(result.template?.sections).toEqual([
      { title: "Updates", description: "Recent changes" },
      { title: "Next Steps", description: "Follow-ups" },
    ]);
  });

  it("keeps the applied template when the memo has no section headings", async () => {
    const result = await runTransform(
      {
        rawTemplateId: "template-1",
        rawContent: JSON.stringify({
          type: "doc",
          content: [{ type: "paragraph" }],
        }),
        rawContentFormat: "prosemirror_json",
        rawMarkdown: "Notes without headings",
      },
      {
        title: "1:1 Meeting",
        description: "Weekly conversation",
        sections: [
          { title: "Updates", description: "Recent changes" },
          { title: "Action Items", description: "Follow-ups" },
        ],
      },
    );

    expect(result.template).toEqual({
      title: "1:1 Meeting",
      description: "Weekly conversation",
      sections: [
        { title: "Updates", description: "Recent changes" },
        { title: "Action Items", description: "Follow-ups" },
      ],
    });
  });

  it("uses the saved format override for Auto summaries", async () => {
    const result = await enhanceTransform.transformArgs(
      { sessionId: "session-1", enhancedNoteId: "note-1" },
      {
        ...settingsValues,
        auto_summary_prompt: "  Start with decisions.  ",
      },
    );

    expect(result.formatOverride).toBe("  Start with decisions.  ");
  });

  it("ignores the Auto override when a named template is selected", async () => {
    const result = await enhanceTransform.transformArgs(
      {
        sessionId: "session-1",
        enhancedNoteId: "note-1",
        templateId: "template-1",
      },
      {
        ...settingsValues,
        auto_summary_prompt: "Start with decisions.",
      },
    );

    expect(result.formatOverride).toBe("");
  });

  it("uses the built-in Auto format when no override is saved", async () => {
    const result = await enhanceTransform.transformArgs(
      { sessionId: "session-1", enhancedNoteId: "note-1" },
      settingsValues,
    );

    expect(result.formatOverride).toBe("");
    expect(result.summaryLength).toBe("detailed");
  });

  it("uses the saved summary length mode", async () => {
    const result = await enhanceTransform.transformArgs(
      { sessionId: "session-1", enhancedNoteId: "note-1" },
      { ...settingsValues, summary_length: "crisp" },
    );

    expect(result.summaryLength).toBe("crisp");
  });

  it("builds the summary policy from the returned transcript segments", async () => {
    const lengthPolicy = {
      mode: "crisp",
      transcript_characters: 27,
      guidance: {
        max_characters: 320,
        min_sections: 1,
        max_sections: 2,
      },
    };
    mocks.renderSessionTranscript.mockResolvedValue({
      status: "ok",
      data: {
        segments: [
          {
            speaker_label: "Alice",
            start_ms: 0,
            end_ms: 10,
            text: "First segment",
            words: [{ text: "First", start_ms: 0, end_ms: 5 }],
          },
          {
            speaker_label: "Alice",
            start_ms: 10,
            end_ms: 20,
            text: "Second segment",
            words: [{ text: "Second", start_ms: 10, end_ms: 15 }],
          },
        ],
      },
    });
    mocks.summaryLengthPolicy.mockResolvedValue({
      status: "ok",
      data: lengthPolicy,
    });

    const result = await enhanceTransform.transformArgs(
      { sessionId: "session-1", enhancedNoteId: "note-1" },
      {
        ...settingsValues,
        summary_length: "crisp",
        auto_summary_prompt: "  Use concise prose.  ",
      },
    );

    expect(mocks.summaryLengthPolicy).toHaveBeenCalledWith({
      transcript_texts: ["First segment", "Second segment"],
      mode: "crisp",
      template_section_count: 0,
    });
    expect(result.lengthPolicy).toEqual(lengthPolicy);
  });

  it("uses the dominant spoken language of transcript text for summaries", async () => {
    mocks.renderSessionTranscript.mockResolvedValue({
      status: "ok",
      data: {
        segments: [
          {
            speaker_label: "Alice",
            start_ms: 0,
            end_ms: 10,
            text: "Transcript segment in English",
            words: [{ text: "Transcript", start_ms: 0, end_ms: 5 }],
          },
        ],
      },
    });
    mocks.dominantLanguage.mockResolvedValue({ status: "ok", data: "ko" });

    const result = await enhanceTransform.transformArgs(
      { sessionId: "session-1", enhancedNoteId: "note-1" },
      { ...settingsValues, spoken_languages: '["ko"]' },
    );

    expect(mocks.dominantLanguage).toHaveBeenCalledWith({
      texts: ["Transcript segment in English"],
      candidates: ["en", "ko"],
    });
    expect(result.language).toBe("ko");
  });

  it("keeps the main language when no additional spoken languages are set", async () => {
    const result = await enhanceTransform.transformArgs(
      { sessionId: "session-1", enhancedNoteId: "note-1" },
      { ...settingsValues, spoken_languages: "[]" },
    );

    expect(mocks.dominantLanguage).not.toHaveBeenCalled();
    expect(result.language).toBe("en");
  });

  it("includes personalization dictionary terms for summary spelling", async () => {
    const result = await enhanceTransform.transformArgs(
      { sessionId: "session-1", enhancedNoteId: "note-1" },
      {
        ...settingsValues,
        personalization_dictionary_terms: JSON.stringify(["Anarlog", "Char"]),
      },
    );

    expect(result.dictionaryTerms).toEqual(["Anarlog", "Char"]);
  });

  it("falls back to generic enhancement when template loading fails", async () => {
    mocks.getTemplateById.mockRejectedValue(new Error("Failed query"));

    const result = await enhanceTransform.transformArgs(
      {
        sessionId: "session-1",
        enhancedNoteId: "note-1",
        templateId: "template-1",
      },
      settingsValues,
    );

    expect(result.template).toBeNull();
    expect(result.formatOverride).toBe("");
    expect(result.session.title).toBe("Weekly Review");
    expect(consoleError).toHaveBeenCalledWith(
      "[enhance] failed to load template",
      expect.any(Error),
    );
  });

  it("collects image context from canonical transcript and note content", async () => {
    await enhanceTransform.transformArgs(
      {
        sessionId: "session-1",
        enhancedNoteId: "note-1",
      },
      {
        current_llm_provider: "openai",
        current_llm_model: "gpt-4o",
        ai_language: "en",
      },
    );

    expect(mocks.collectEnhanceImageContext).toHaveBeenCalledWith("session-1", [
      "![pre](asset://localhost/pre.png)",
      "![post](asset://localhost/post.png)",
    ]);
  });

  it("uses Rust-rendered transcript segments in summary context", async () => {
    mocks.renderSessionTranscript.mockResolvedValue({
      status: "ok",
      data: {
        segments: [
          {
            id: "later",
            key: { channel: "DirectMic" },
            speaker_label: "Later speaker",
            start_ms: 200,
            end_ms: 300,
            text: "later words",
            words: [
              {
                text: "later words",
                start_ms: 200,
                end_ms: 300,
                channel: "DirectMic",
                is_final: true,
              },
            ],
          },
          {
            id: "empty",
            key: { channel: "DirectMic" },
            speaker_label: "Empty speaker",
            start_ms: 0,
            end_ms: 100,
            text: "",
            words: [],
          },
          {
            id: "earlier",
            key: { channel: "DirectMic" },
            speaker_label: "Earlier speaker",
            start_ms: 100,
            end_ms: 150,
            text: "earlier words",
            words: [
              {
                text: "earlier words",
                start_ms: 100,
                end_ms: 150,
                channel: "DirectMic",
                is_final: true,
              },
            ],
          },
        ],
        started_at: 100,
        ended_at: 200,
      },
    });

    const result = await enhanceTransform.transformArgs(
      { sessionId: "session-1", enhancedNoteId: "note-1" },
      settingsValues,
    );

    expect(result.transcripts).toEqual([
      {
        segments: [
          { speaker: "Later speaker", text: "later words" },
          { speaker: "Earlier speaker", text: "earlier words" },
        ],
        startedAt: 100,
        endedAt: 200,
      },
    ]);
  });

  it("keeps the renderer's channel grouping in generated-note transcript context", async () => {
    mocks.renderSessionTranscript.mockResolvedValue({
      status: "ok",
      data: {
        segments: [
          {
            speaker_label: "Speaker 1",
            text: "Mic early",
            start_ms: 0,
            end_ms: 400,
            words: [{ text: "Mic early", start_ms: 0, end_ms: 400 }],
          },
          {
            speaker_label: "Speaker 1",
            text: "Mic later",
            start_ms: 29_500,
            end_ms: 29_900,
            words: [{ text: "Mic later", start_ms: 29_500, end_ms: 29_900 }],
          },
          {
            speaker_label: "Speaker 2",
            text: "Remote early",
            start_ms: 0,
            end_ms: 400,
            words: [{ text: "Remote early", start_ms: 0, end_ms: 400 }],
          },
        ],
        started_at: 100,
        ended_at: 200,
      },
    });

    const result = await enhanceTransform.transformArgs(
      { sessionId: "session-1", enhancedNoteId: "note-1" },
      settingsValues,
    );

    expect(
      result.transcripts[0]?.segments.map((segment) => segment.text),
    ).toEqual(["Mic early", "Mic later", "Remote early"]);
  });

  it("keeps timed two-channel transcripts in start order", async () => {
    mocks.renderSessionTranscript.mockResolvedValue({
      status: "ok",
      data: {
        segments: [
          {
            speaker_label: "Speaker 1",
            text: "Mic first",
            start_ms: 0,
            end_ms: 400,
            words: [{ text: "Mic first", start_ms: 0, end_ms: 400 }],
          },
          {
            speaker_label: "Speaker 2",
            text: "Remote second",
            start_ms: 500,
            end_ms: 900,
            words: [{ text: "Remote second", start_ms: 500, end_ms: 900 }],
          },
          {
            speaker_label: "Speaker 1",
            text: "Mic third",
            start_ms: 1_000,
            end_ms: 1_400,
            words: [{ text: "Mic third", start_ms: 1_000, end_ms: 1_400 }],
          },
        ],
        started_at: 100,
        ended_at: 200,
      },
    });

    const result = await enhanceTransform.transformArgs(
      { sessionId: "session-1", enhancedNoteId: "note-1" },
      settingsValues,
    );

    expect(
      result.transcripts[0]?.segments.map((segment) => segment.text),
    ).toEqual(["Mic first", "Remote second", "Mic third"]);
  });

  it("includes captured meeting chat in the post-meeting memo", async () => {
    mocks.loadMeetingChatRecords.mockResolvedValue([
      { text: "Review the rollout plan" },
    ]);
    mocks.formatMeetingChatContext.mockReturnValue(
      "## Meeting chat\n- Slack · Ada\n  Review the rollout plan",
    );

    const result = await enhanceTransform.transformArgs(
      { sessionId: "session-1", enhancedNoteId: "note-1" },
      settingsValues,
    );

    expect(result.postMeetingMemo).toBe(
      "![post](asset://localhost/post.png)\n\n## Meeting chat\n- Slack · Ada\n  Review the rollout plan",
    );
  });

  it("rejects generation when the session no longer exists", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue(null);

    await expect(
      enhanceTransform.transformArgs(
        { sessionId: "missing", enhancedNoteId: "note-1" },
        settingsValues,
      ),
    ).rejects.toThrow("Session missing no longer exists");
  });
});
