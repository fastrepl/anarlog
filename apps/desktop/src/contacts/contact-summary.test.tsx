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
  createContactSummaryPromptKey,
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
    const promptKey = createContactSummaryPromptKey(null);
    const first = createContactSummarySourceHash(sessions, promptKey);

    expect(first).toBe(createContactSummarySourceHash(sessions, promptKey));
    expect(
      createContactSummarySourceHash(
        [{ ...sessions[0]!, sourceUpdatedAt: "2026-08-12T13:00:00.000Z" }],
        promptKey,
      ),
    ).not.toBe(first);
  });

  it("changes the source fingerprint when the user identity changes", () => {
    const sessions = makeSessions();
    const user = makeUser();
    const withUser = createContactSummarySourceHash(
      sessions,
      createContactSummaryPromptKey(user),
    );

    const withoutUser = createContactSummarySourceHash(
      sessions,
      createContactSummaryPromptKey(null),
    );
    expect(withoutUser).not.toBe(withUser);
    // A self contact with blank name and email is still a known user.
    expect(
      createContactSummarySourceHash(
        sessions,
        createContactSummaryPromptKey({ ...user, name: "", email: "" }),
      ),
    ).not.toBe(withoutUser);
    for (const changed of [
      { ...user, id: "user-2" },
      { ...user, name: "Johnny" },
      { ...user, email: "john@other.com" },
      { ...user, name: "", email: "" },
    ]) {
      expect(
        createContactSummarySourceHash(
          sessions,
          createContactSummaryPromptKey(changed),
        ),
      ).not.toBe(withUser);
    }
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

    expect(mocks.updateHumanContactSummary).toHaveBeenCalledWith(
      "human-1",
      expect.objectContaining({
        sourceHash: "source-1",
        promptKey: createContactSummaryPromptKey(null),
        sources: [{ id: "session-1", updatedAt: "2026-08-11T12:00:00.000Z" }],
      }),
    );
  });

  it("includes the user context and addresses them in the second person", async () => {
    const user = makeUser();

    await generateAndSaveContactSummary({
      human: makeHuman(),
      user,
      organizationName: "Fastrepl",
      sessions: makeSessions(),
      sourceHash: "source-1",
      model: { id: "model-1" } as never,
    });

    const call = mocks.generateText.mock.calls[0]?.[0];
    expect(call.system).toContain("second person");
    const prompt = JSON.parse(call.prompt);
    expect(prompt.user).toEqual({
      name: "John",
      email: "john@example.com",
    });
    expect(prompt.target_is_user).toBe(false);
  });

  it("marks the brief as about the user when target and user match", async () => {
    const user = makeUser();

    await generateAndSaveContactSummary({
      human: user,
      user,
      organizationName: "Fastrepl",
      sessions: makeSessions(),
      sourceHash: "source-1",
      model: { id: "model-1" } as never,
    });

    const prompt = JSON.parse(mocks.generateText.mock.calls[0]?.[0].prompt);
    expect(prompt.target_is_user).toBe(true);
  });

  it("sends a null user when no user contact is known", async () => {
    await generateAndSaveContactSummary({
      human: makeHuman(),
      organizationName: "Fastrepl",
      sessions: makeSessions(),
      sourceHash: "source-1",
      model: { id: "model-1" } as never,
    });

    const prompt = JSON.parse(mocks.generateText.mock.calls[0]?.[0].prompt);
    expect(prompt.user).toBeNull();
    expect(prompt.target_is_user).toBe(false);
  });

  it("rebuilds in full when only the prompt version changed the hash", async () => {
    const sessions = makeSessions();
    const human = {
      ...makeHuman(),
      summary: {
        facts: ["Fact one.", "Fact two.", "Fact three."],
        sourceHash: "source-old-version",
        promptKey: "",
        generatedAt: "2026-08-11T12:00:00.000Z",
        sources: [{ id: "session-1", updatedAt: "2026-08-11T12:00:00.000Z" }],
      },
    };

    await generateAndSaveContactSummary({
      human,
      organizationName: "Fastrepl",
      sessions,
      sourceHash: "source-2",
      model: { id: "model-1" } as never,
    });

    expect(mocks.loadSessionContentSnapshot).toHaveBeenCalledTimes(1);
    const prompt = JSON.parse(mocks.generateText.mock.calls[0]?.[0].prompt);
    expect(prompt.existing_facts).toBeUndefined();
  });

  it("rebuilds a legacy brief in full even when a new meeting arrives", async () => {
    const human = {
      ...makeHuman(),
      summary: {
        facts: ["Fact one.", "Fact two.", "Fact three."],
        sourceHash: "source-old",
        promptKey: "",
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

    expect(mocks.loadSessionContentSnapshot).toHaveBeenCalledTimes(2);
    const prompt = JSON.parse(mocks.generateText.mock.calls[0]?.[0].prompt);
    expect(prompt.existing_facts).toBeUndefined();
  });

  it("extends an existing summary with only the new meetings", async () => {
    const human = {
      ...makeHuman(),
      summary: {
        facts: ["Fact one.", "Fact two.", "Fact three."],
        sourceHash: "source-old",
        promptKey: createContactSummaryPromptKey(null),
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

  it.each([
    {
      change: "a summarized meeting was removed",
      sources: [
        { id: "session-0", updatedAt: "2026-08-05T12:00:00.000Z" },
        { id: "session-1", updatedAt: "2026-08-11T12:00:00.000Z" },
      ],
    },
    {
      change: "an already-summarized meeting changed",
      sources: [{ id: "session-1", updatedAt: "2026-08-01T12:00:00.000Z" }],
    },
  ])("rebuilds from scratch when $change", async ({ sources }) => {
    const human = {
      ...makeHuman(),
      summary: {
        facts: ["Fact one.", "Fact two.", "Fact three."],
        sourceHash: "source-old",
        promptKey: createContactSummaryPromptKey(null),
        generatedAt: "2026-08-11T12:00:00.000Z",
        sources,
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

  it("does not restart generation while session updates keep arriving", async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );

    let resolveGeneration!: (value: { output: { facts: string[] } }) => void;
    mocks.generateText.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveGeneration = resolve;
        }),
    );

    const props = {
      human: makeHuman(),
      user: makeUser(),
      organizationName: "Fastrepl",
      sessions: makeSessions(),
      settleMs: 200,
    };
    const { rerender, result } = renderHook(
      (nextProps: typeof props) => useContactSummary(nextProps),
      { wrapper, initialProps: props },
    );

    await waitFor(() => {
      expect(mocks.generateText).toHaveBeenCalledOnce();
    });

    // Writes landing in the same burst emit once at the leading edge, so a
    // single restart picks up the newest fingerprint...
    rerender({
      ...props,
      sessions: makeSessions().map((session) => ({
        ...session,
        sourceUpdatedAt: "2026-08-11T12:00:01.000Z",
      })),
    });
    await waitFor(() => {
      expect(mocks.generateText).toHaveBeenCalledTimes(2);
    });
    await waitFor(() => {
      expect(result.current.isGenerating).toBe(false);
    });

    // ...and further continuous writes (recording, enhance, edits) spanning
    // several settle windows emit nothing until the source goes quiet.
    for (let bump = 2; bump <= 4; bump++) {
      rerender({
        ...props,
        sessions: makeSessions().map((session) => ({
          ...session,
          sourceUpdatedAt: `2026-08-11T12:00:0${bump}.000Z`,
        })),
      });
      await new Promise((resolve) => setTimeout(resolve, 120));
    }
    expect(mocks.generateText).toHaveBeenCalledTimes(2);

    // Once the source goes quiet, exactly one follow-up run picks up the
    // newest fingerprint instead of restarting per write.
    mocks.generateText.mockResolvedValue({
      output: {
        facts: ["A.", "B.", "C."],
      },
    });
    await waitFor(() => {
      expect(mocks.generateText).toHaveBeenCalledTimes(3);
    });
    await waitFor(() => {
      expect(result.current.isGenerating).toBe(false);
    });

    // The superseded run resolving late must not overwrite the newer saved
    // summary: it is aborted, so it never reaches the write.
    resolveGeneration({
      output: { facts: ["Stale.", "Facts.", "Here."] },
    });
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(mocks.updateHumanContactSummary).toHaveBeenCalledTimes(2);
    expect(mocks.updateHumanContactSummary).toHaveBeenLastCalledWith(
      "human-1",
      expect.objectContaining({
        sourceHash: createContactSummarySourceHash(
          makeSessions().map((session) => ({
            ...session,
            sourceUpdatedAt: "2026-08-11T12:00:04.000Z",
          })),
          createContactSummaryPromptKey(makeUser()),
        ),
      }),
    );
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
          user: makeUser(),
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
    additionalEmails: [],
  };
}

function makeUser(): HumanRecord {
  return {
    ...makeHuman(),
    id: "user-1",
    name: "John",
    email: "john@example.com",
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
    rawUpdatedAt: "",
    rawHeadingsUpdatedAt: "",
    rawAppliedTemplate: null,
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
