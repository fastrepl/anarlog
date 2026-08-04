import type { LanguageModel } from "ai";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { TaskConfig } from ".";
import {
  getPersistableGeneratedTitle,
  persistGeneratedTitle,
  titleSuccess,
} from "./title-success";

import { useLiveTitle } from "~/store/zustand/live-title";

const mocks = vi.hoisted(() => ({
  loadSessionContentSnapshot: vi.fn(),
  applyGeneratedSessionTitle: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("~/session/content-queries", () => ({
  loadSessionContentSnapshot: mocks.loadSessionContentSnapshot,
}));

vi.mock("~/session/content-mutations", () => ({
  applyGeneratedSessionTitle: mocks.applyGeneratedSessionTitle,
}));

type TitleSuccessParams = Parameters<
  NonNullable<TaskConfig<"title">["onSuccess"]>
>[0];

function createSnapshot(title = "") {
  return {
    sessionId: "session-1",
    ownerUserId: "user-1",
    title,
    createdAt: "2026-07-10T00:00:00.000Z",
    event: null,
    eventId: null,
    rawNoteId: "session-1",
    rawContent: "Raw note",
    rawContentFormat: "markdown",
    rawMarkdown: "Raw note",
    enhancedNotes: [
      {
        id: "note-1",
        title: "",
        markdown: "# Summary section",
        content: "# Summary section",
        contentFormat: "markdown",
        templateId: "",
        position: 0,
      },
    ],
    transcripts: [],
    participants: [],
  };
}

function createParams(
  overrides: Partial<TitleSuccessParams> = {},
): TitleSuccessParams {
  return {
    taskId: "session-1-title",
    text: "Meeting title",
    model: {} as LanguageModel,
    args: { sessionId: "session-1" },
    transformedArgs: {} as TitleSuccessParams["transformedArgs"],
    signal: new AbortController().signal,
    startTask: vi.fn().mockResolvedValue(undefined),
    getTaskState: vi.fn().mockReturnValue(undefined),
    ...overrides,
  };
}

describe("titleSuccess.onSuccess", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useLiveTitle.setState({ titles: {} });
    mocks.loadSessionContentSnapshot.mockResolvedValue(createSnapshot());
    mocks.applyGeneratedSessionTitle.mockResolvedValue(undefined);
  });

  it("persists a trimmed title without adding it to the memo", async () => {
    await titleSuccess.onSuccess?.(createParams({ text: "  Weekly sync  " }));

    expect(mocks.applyGeneratedSessionTitle).toHaveBeenCalledWith({
      sessionId: "session-1",
      currentTitle: "",
      nextTitle: "Weekly sync",
      documents: [
        expect.objectContaining({
          id: "note-1",
          currentContent: "# Summary section",
          currentContentFormat: "markdown",
          nextContent: expect.stringContaining("Weekly sync"),
        }),
      ],
    });
  });

  it("persists only the final title when model reasoning leaks into text", async () => {
    await titleSuccess.onSuccess?.(
      createParams({
        text: [
          "We need to output a super concise title about the meeting.",
          "The note discusses bore pit and wetland compliance.",
          "",
          "Bore Pit Wetland Compliance and Design Standards",
        ].join("\n"),
      }),
    );

    expect(mocks.applyGeneratedSessionTitle).toHaveBeenCalledWith({
      sessionId: "session-1",
      currentTitle: "",
      nextTitle: "Bore Pit Wetland Compliance and Design Standards",
      documents: [
        expect.objectContaining({
          id: "note-1",
          nextContent: expect.not.stringContaining(
            "We need to output a super concise title",
          ),
        }),
      ],
    });
  });

  it("does not overwrite an existing session title", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue(
      createSnapshot("Custom title"),
    );

    await expect(
      persistGeneratedTitle({
        text: "Generated title",
        args: { sessionId: "session-1" },
      }),
    ).resolves.toBe(false);
    expect(mocks.applyGeneratedSessionTitle).not.toHaveBeenCalled();
  });

  it("does not overwrite an active title edit, including a blank draft", async () => {
    useLiveTitle.getState().setTitle("session-1", "");

    await titleSuccess.onSuccess?.(createParams());

    expect(mocks.loadSessionContentSnapshot).not.toHaveBeenCalled();
    expect(mocks.applyGeneratedSessionTitle).not.toHaveBeenCalled();
  });

  it("ignores empty and placeholder title outputs", async () => {
    await titleSuccess.onSuccess?.(createParams({ text: "   " }));
    await titleSuccess.onSuccess?.(createParams({ text: "<EMPTY>" }));

    expect(mocks.loadSessionContentSnapshot).not.toHaveBeenCalled();
    expect(mocks.applyGeneratedSessionTitle).not.toHaveBeenCalled();
  });

  it("does not persist a title task marked skipPersist", async () => {
    await titleSuccess.onSuccess?.(
      createParams({ args: { sessionId: "session-1", skipPersist: true } }),
    );

    expect(mocks.loadSessionContentSnapshot).not.toHaveBeenCalled();
  });

  it("propagates a stale transaction failure", async () => {
    mocks.applyGeneratedSessionTitle.mockRejectedValueOnce(
      new Error("unexpected rows affected"),
    );

    await expect(titleSuccess.onSuccess?.(createParams())).rejects.toThrow(
      "unexpected rows affected",
    );
  });
});

describe("getPersistableGeneratedTitle", () => {
  it("normalizes common model response wrappers", () => {
    const wrappedTitles = [
      'Final title: **"Weekly Design Review"**',
      '"**Weekly Design Review**"',
      '"Final title: Weekly Design Review"',
    ];

    for (const wrappedTitle of wrappedTitles) {
      expect(
        getPersistableGeneratedTitle(
          `Reasoning about the response\n${wrappedTitle}`,
        ),
      ).toBe("Weekly Design Review");
    }
  });

  it("preserves unmatched and inline title markers", () => {
    const titles = [
      "__init__ design review",
      'Review "quoted" results',
      "Discuss **urgent** follow-up",
    ];

    for (const title of titles) {
      expect(getPersistableGeneratedTitle(`Final title: ${title}`)).toBe(title);
    }
  });

  it("rejects placeholders and oversized output", () => {
    expect(getPersistableGeneratedTitle('"<EMPTY>"')).toBe("");
    expect(getPersistableGeneratedTitle("x".repeat(161))).toBe("");
  });
});
