import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { enhanceTransform } from "./enhance-transform";

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
    mocks.loadSessionContentSnapshot.mockResolvedValue({
      ...createSnapshot(),
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
    });
    mocks.getTemplateById.mockResolvedValue({
      title: "1:1 Meeting",
      description: "Weekly conversation",
      sections: [
        { title: "Updates", description: "Recent changes" },
        { title: "Action Items", description: "Follow-ups" },
      ],
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
      title: "1:1 Meeting",
      description: "Weekly conversation",
      sections: [
        { title: "Updates", description: "Recent changes" },
        { title: "Next Steps", description: "Follow-ups" },
      ],
    });
  });

  it("keeps the applied template when the memo has no section headings", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue({
      ...createSnapshot(),
      rawTemplateId: "template-1",
      rawContent: JSON.stringify({
        type: "doc",
        content: [{ type: "paragraph" }],
      }),
      rawContentFormat: "prosemirror_json",
      rawMarkdown: "Notes without headings",
    });
    mocks.getTemplateById.mockResolvedValue({
      title: "1:1 Meeting",
      description: "Weekly conversation",
      sections: [
        { title: "Updates", description: "Recent changes" },
        { title: "Action Items", description: "Follow-ups" },
      ],
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
  });

  it("builds the summary policy from the returned transcript segments", async () => {
    const lengthPolicy = {
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
        auto_summary_prompt: "  Use concise prose.  ",
      },
    );

    expect(mocks.summaryLengthPolicy).toHaveBeenCalledWith({
      transcript_texts: ["First segment", "Second segment"],
      template_section_count: 0,
    });
    expect(result.lengthPolicy).toEqual(lengthPolicy);
  });

  it.each([
    { summaryUseMainLanguage: undefined, expected: "ko" },
    { summaryUseMainLanguage: true, expected: "en" },
  ])(
    "uses $expected for summaries with Korean transcription enabled (main language override: $summaryUseMainLanguage)",
    async ({ summaryUseMainLanguage, expected }) => {
      mocks.renderSessionTranscript.mockResolvedValue({
        status: "ok",
        data: {
          segments: [
            {
              speaker_label: "Alice",
              start_ms: 0,
              end_ms: 10,
              text: "오늘 회의에 참석해 주셔서 감사합니다.",
              words: [{ text: "오늘", start_ms: 0, end_ms: 5 }],
            },
          ],
        },
      });
      mocks.dominantLanguage.mockResolvedValue({ status: "ok", data: "ko" });

      const result = await enhanceTransform.transformArgs(
        { sessionId: "session-1", enhancedNoteId: "note-1" },
        {
          ...settingsValues,
          spoken_languages: '["ko"]',
          summary_use_main_language: summaryUseMainLanguage,
        },
      );

      expect(result.language).toBe(expected);
    },
  );

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
          { speaker: "Later speaker", text: "later words", startMs: 200 },
          { speaker: "Earlier speaker", text: "earlier words", startMs: 100 },
        ],
        startedAt: 100,
        endedAt: 200,
        nowMs: null,
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
