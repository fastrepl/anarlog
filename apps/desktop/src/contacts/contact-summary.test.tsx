import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { SessionContentSnapshot } from "~/session/content-queries";

const mocks = vi.hoisted(() => ({
  generateText: vi.fn(),
  loadSessionContentSnapshot: vi.fn(),
  updateHumanContactSummary: vi.fn(() => Promise.resolve()),
}));

vi.mock("ai", async (importOriginal) => ({
  ...(await importOriginal<typeof import("ai")>()),
  generateText: mocks.generateText,
}));

vi.mock("~/ai/hooks", () => ({
  useLanguageModel: () => ({ id: "model-1" }),
}));

vi.mock("~/session/content-queries", () => ({
  loadSessionContentSnapshot: mocks.loadSessionContentSnapshot,
}));

vi.mock("./queries", () => ({
  updateHumanContactSummary: mocks.updateHumanContactSummary,
}));

import {
  buildContactSummarySource,
  createContactSummarySourceHash,
  generateAndSaveContactSummary,
  useContactSummary,
} from "./contact-summary";
import type { HumanRecord, HumanSessionRecord } from "./queries";

beforeEach(() => {
  vi.clearAllMocks();
  mocks.generateText.mockResolvedValue({
    output: {
      facts: [
        "- Prefers concise weekly updates.",
        "2. Owns the launch timeline.",
        "Needs pricing by Friday.",
      ],
    },
  });
  mocks.loadSessionContentSnapshot.mockResolvedValue(
    makeSnapshot({
      sessionId: "session-1",
      title: "Launch planning",
      createdAt: "2026-08-10T12:00:00.000Z",
      summary: "Alice owns the launch timeline and needs pricing by Friday.",
    }),
  );
});

describe("contact summary", () => {
  it("changes the source fingerprint when related meeting content changes", () => {
    const sessions = makeSessions();
    const first = createContactSummarySourceHash(sessions);

    expect(first).toBe(createContactSummarySourceHash(sessions));
    expect(
      createContactSummarySourceHash([
        { ...sessions[0]!, sourceUpdatedAt: "2026-08-12T13:00:00.000Z" },
      ]),
    ).not.toBe(first);
  });

  it("uses generated meeting summaries and transcript fallbacks", () => {
    const sources = buildContactSummarySource([
      makeSnapshot({
        sessionId: "session-1",
        title: "Recent planning",
        createdAt: "2026-08-10T12:00:00.000Z",
        summary: "Alice owns launch planning.",
      }),
      makeSnapshot({
        sessionId: "session-2",
        title: "Earlier call",
        createdAt: "2026-08-01T12:00:00.000Z",
        transcript: "Alice prefers weekly updates.",
      }),
    ]);

    expect(sources).toEqual([
      expect.objectContaining({
        sessionId: "session-1",
        content: "Summary: Alice owns launch planning.",
      }),
      expect.objectContaining({
        sessionId: "session-2",
        content: "Transcript: Alice prefers weekly updates.",
      }),
    ]);
  });

  it("persists at least three normalized facts with the source fingerprint", async () => {
    const human = makeHuman();
    const sessions = makeSessions();

    await expect(
      generateAndSaveContactSummary({
        human,
        organizationName: "Fastrepl",
        sessions,
        sourceHash: "source-1",
        model: { id: "model-1" } as never,
      }),
    ).resolves.toEqual(
      expect.objectContaining({
        facts: [
          "Prefers concise weekly updates.",
          "Owns the launch timeline.",
          "Needs pricing by Friday.",
        ],
        sourceHash: "source-1",
      }),
    );

    expect(mocks.generateText.mock.calls[0]?.[0]).toMatchObject({
      maxOutputTokens: 4_096,
      timeout: { totalMs: 45_000 },
    });
    expect(mocks.generateText.mock.calls[0]?.[0].system).toContain(
      "Prefer newer evidence",
    );
    expect(mocks.updateHumanContactSummary).toHaveBeenCalledWith(
      "human-1",
      expect.objectContaining({
        sourceHash: "source-1",
        sources: [{ id: "session-1", updatedAt: "2026-08-11T12:00:00.000Z" }],
      }),
    );
  });

  it("extends an existing summary with only the new meetings", async () => {
    const human = {
      ...makeHuman(),
      summary: {
        facts: ["Fact one.", "Fact two.", "Fact three."],
        sourceHash: "source-old",
        generatedAt: "2026-08-11T12:00:00.000Z",
        sources: [{ id: "session-1", updatedAt: "2026-08-11T12:00:00.000Z" }],
      },
    };
    const sessions: HumanSessionRecord[] = [
      {
        id: "session-2",
        title: "Follow-up",
        createdAt: "2026-08-15T12:00:00.000Z",
        sourceUpdatedAt: "2026-08-15T13:00:00.000Z",
      },
      ...makeSessions(),
    ];

    await generateAndSaveContactSummary({
      human,
      organizationName: "Fastrepl",
      sessions,
      sourceHash: "source-2",
      model: { id: "model-1" } as never,
    });

    expect(mocks.loadSessionContentSnapshot).toHaveBeenCalledTimes(1);
    expect(mocks.loadSessionContentSnapshot).toHaveBeenCalledWith("session-2");
    const prompt = JSON.parse(mocks.generateText.mock.calls[0]?.[0].prompt);
    expect(prompt.existing_facts).toEqual([
      "Fact one.",
      "Fact two.",
      "Fact three.",
    ]);
  });

  it("rebuilds from scratch when a summarized meeting was removed", async () => {
    const human = {
      ...makeHuman(),
      summary: {
        facts: ["Fact one.", "Fact two.", "Fact three."],
        sourceHash: "source-old",
        generatedAt: "2026-08-11T12:00:00.000Z",
        sources: [
          { id: "session-0", updatedAt: "2026-08-05T12:00:00.000Z" },
          { id: "session-1", updatedAt: "2026-08-11T12:00:00.000Z" },
        ],
      },
    };
    const sessions: HumanSessionRecord[] = [
      {
        id: "session-2",
        title: "Follow-up",
        createdAt: "2026-08-15T12:00:00.000Z",
        sourceUpdatedAt: "2026-08-15T13:00:00.000Z",
      },
      ...makeSessions(),
    ];

    await generateAndSaveContactSummary({
      human,
      organizationName: "Fastrepl",
      sessions,
      sourceHash: "source-2",
      model: { id: "model-1" } as never,
    });

    expect(mocks.loadSessionContentSnapshot).toHaveBeenCalledTimes(2);
    const prompt = JSON.parse(mocks.generateText.mock.calls[0]?.[0].prompt);
    expect(prompt.existing_facts).toBeUndefined();
  });

  it("rebuilds from scratch when an already-summarized meeting changed", async () => {
    const human = {
      ...makeHuman(),
      summary: {
        facts: ["Fact one.", "Fact two.", "Fact three."],
        sourceHash: "source-old",
        generatedAt: "2026-08-11T12:00:00.000Z",
        sources: [{ id: "session-1", updatedAt: "2026-08-01T12:00:00.000Z" }],
      },
    };
    const sessions: HumanSessionRecord[] = [
      {
        id: "session-2",
        title: "Follow-up",
        createdAt: "2026-08-15T12:00:00.000Z",
        sourceUpdatedAt: "2026-08-15T13:00:00.000Z",
      },
      ...makeSessions(),
    ];

    await generateAndSaveContactSummary({
      human,
      organizationName: "Fastrepl",
      sessions,
      sourceHash: "source-2",
      model: { id: "model-1" } as never,
    });

    expect(mocks.loadSessionContentSnapshot).toHaveBeenCalledTimes(2);
    const prompt = JSON.parse(mocks.generateText.mock.calls[0]?.[0].prompt);
    expect(prompt.existing_facts).toBeUndefined();
  });

  it("automatically generates a stale summary when the contact is viewed", async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );

    const { result } = renderHook(
      () =>
        useContactSummary({
          human: makeHuman(),
          organizationName: "Fastrepl",
          sessions: makeSessions(),
        }),
      { wrapper },
    );

    await waitFor(() => {
      expect(result.current.facts).toHaveLength(3);
    });
    expect(mocks.generateText).toHaveBeenCalledOnce();
    expect(mocks.updateHumanContactSummary).toHaveBeenCalledOnce();
  });
});

function makeHuman(): HumanRecord {
  return {
    id: "human-1",
    userId: "user-1",
    createdAt: "2026-08-01T12:00:00.000Z",
    organizationId: "organization-1",
    name: "Alice",
    email: "alice@example.com",
    phone: "",
    jobTitle: "Founder",
    linkedinUsername: "",
    memo: "Prefers concise weekly updates.",
    pinned: false,
    pinOrder: null,
    avatarDataUrl: null,
    summary: null,
  };
}

function makeSessions(): HumanSessionRecord[] {
  return [
    {
      id: "session-1",
      title: "Launch planning",
      createdAt: "2026-08-10T12:00:00.000Z",
      sourceUpdatedAt: "2026-08-11T12:00:00.000Z",
    },
  ];
}

function makeSnapshot({
  sessionId,
  title,
  createdAt,
  summary = "",
  transcript = "",
}: {
  sessionId: string;
  title: string;
  createdAt: string;
  summary?: string;
  transcript?: string;
}): SessionContentSnapshot {
  return {
    sessionId,
    ownerUserId: "user-1",
    title,
    createdAt,
    event: null,
    sourceApps: [],
    eventId: null,
    rawNoteId: null,
    rawTemplateId: "",
    rawContent: "",
    rawContentFormat: "markdown",
    rawMarkdown: "",
    enhancedNotes: summary
      ? [
          {
            id: `${sessionId}:summary`,
            title: "Summary",
            markdown: summary,
            content: summary,
            contentFormat: "markdown",
            templateId: "",
            position: 0,
          },
        ]
      : [],
    transcripts: transcript
      ? [
          {
            id: `${sessionId}:transcript`,
            started_at: 0,
            ended_at: 1,
            memo: "",
            wordsJson: "[]",
            speakerHintsJson: "[]",
            words: [{ id: "word-1", text: transcript } as never],
            speaker_hints: [],
          },
        ]
      : [],
    participants: [{ humanId: "human-1", name: "Alice", jobTitle: "Founder" }],
  };
}
