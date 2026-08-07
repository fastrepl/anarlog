import type { LanguageModel } from "ai";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  generateText: vi.fn(),
}));

vi.mock("ai", async (importOriginal) => ({
  ...(await importOriginal<typeof import("ai")>()),
  generateText: mocks.generateText,
}));

import { inferAutomaticSpeakerAssignments } from "./speaker-attribution";

import type { SessionContentSnapshot } from "~/session/content-queries";

function createSnapshot(channel = 1): SessionContentSnapshot {
  const speakerHints = [
    {
      id: "lex-1:provider_speaker_index",
      word_id: "lex-1",
      type: "provider_speaker_index",
      value: JSON.stringify({ channel, speaker_index: 0 }),
    },
    {
      id: "lex-2:provider_speaker_index",
      word_id: "lex-2",
      type: "provider_speaker_index",
      value: JSON.stringify({ channel, speaker_index: 0 }),
    },
    {
      id: "george-1:provider_speaker_index",
      word_id: "george-1",
      type: "provider_speaker_index",
      value: JSON.stringify({ channel, speaker_index: 1 }),
    },
    {
      id: "george-2:provider_speaker_index",
      word_id: "george-2",
      type: "provider_speaker_index",
      value: JSON.stringify({ channel, speaker_index: 1 }),
    },
  ];

  return {
    sessionId: "session-1",
    ownerUserId: "self",
    title: "Open source AI",
    createdAt: "2026-07-28T00:00:00.000Z",
    event: null,
    eventId: null,
    rawNoteId: "session-1",
    rawTemplateId: "",
    rawContent: "",
    rawContentFormat: "prosemirror_json",
    rawMarkdown: "",
    enhancedNotes: [],
    transcripts: [
      {
        id: "transcript-1",
        started_at: 0,
        ended_at: 2_000,
        memo: "",
        wordsJson: "original words",
        speakerHintsJson: JSON.stringify(speakerHints),
        words: [
          {
            id: "lex-1",
            text: " What do you think about open source Llama",
            start_ms: 0,
            end_ms: 500,
            channel,
          },
          {
            id: "lex-2",
            text: " and the future of AI?",
            start_ms: 500,
            end_ms: 1_000,
            channel,
          },
          {
            id: "george-1",
            text: " Zuckerberg is a good guy and open source matters",
            start_ms: 1_000,
            end_ms: 1_500,
            channel,
          },
          {
            id: "george-2",
            text: " undoubtedly.",
            start_ms: 1_500,
            end_ms: 2_000,
            channel,
          },
        ],
        speaker_hints: speakerHints,
      },
    ],
    participants: [
      { humanId: "human-lex", name: "Lex Fridman", jobTitle: "Host" },
      { humanId: "human-george", name: "George Hotz", jobTitle: "Founder" },
    ],
  };
}

function mockDirectCandidateMatches() {
  mocks.generateText.mockImplementation(({ prompt }: { prompt: string }) => {
    const payload = JSON.parse(prompt) as {
      candidate: { human_id: string };
      clusters: Array<{
        cluster_id: string;
        evidence: { id: string };
      }>;
    };
    const speakerIndex = payload.candidate.human_id === "human-lex" ? 0 : 1;
    const cluster = payload.clusters.find((candidate) =>
      candidate.cluster_id.endsWith(`:${speakerIndex}`),
    )!;

    return Promise.resolve({
      text: `Result:
\`\`\`json
${JSON.stringify({
  mapping: {
    cluster_id: cluster.cluster_id,
    confidence: 0.98,
    evidence_id: cluster.evidence.id,
  },
  candidate: payload.candidate,
})}
\`\`\``,
    });
  });
}

function automaticHumanIds(update: { nextSpeakerHintsJson: string }): string[] {
  return JSON.parse(update.nextSpeakerHintsJson)
    .filter(
      (hint: { type: string }) => hint.type === "automatic_speaker_assignment",
    )
    .map(
      (hint: { value: string }) =>
        (JSON.parse(hint.value) as { human_id: string }).human_id,
    );
}

describe("inferAutomaticSpeakerAssignments", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("creates guarded automatic hints from direct candidate matches", async () => {
    mockDirectCandidateMatches();

    const updates = await inferAutomaticSpeakerAssignments({
      generatedSummary:
        "Lex Fridman asked about Llama. George Hotz said Zuckerberg is a good guy.",
      model: {} as LanguageModel,
      snapshot: createSnapshot(),
      signal: new AbortController().signal,
    });

    expect(mocks.generateText).toHaveBeenCalledTimes(2);
    const georgePrompt = JSON.parse(
      mocks.generateText.mock.calls[0]![0].prompt,
    );
    expect(georgePrompt).toMatchObject({
      candidate: {
        human_id: "human-george",
        summary_mentions: [
          {
            id: "summary-1",
            quote: "George Hotz said Zuckerberg is a good guy.",
          },
        ],
      },
      clusters: [
        {
          cluster_id: "transcript-1:0",
          evidence: {
            id: "evidence-1",
            quote:
              "What do you think about open source Llama and the future of AI?",
          },
        },
        {
          cluster_id: "transcript-1:1",
          evidence: {
            id: "evidence-1",
            quote:
              "Zuckerberg is a good guy and open source matters undoubtedly.",
          },
        },
      ],
    });
    expect(updates).toEqual([
      expect.objectContaining({
        id: "transcript-1",
        currentWordsJson: "original words",
        expectedParticipantHumanIdsJson: '["human-george","human-lex"]',
      }),
    ]);
    expect(automaticHumanIds(updates[0]!)).toEqual([
      "human-lex",
      "human-george",
    ]);
  });

  it("completes the final identity from one exclusive named match", async () => {
    mocks.generateText.mockImplementation(({ prompt }: { prompt: string }) => {
      const payload = JSON.parse(prompt) as {
        candidate: { human_id: string };
        clusters: Array<{
          cluster_id: string;
          evidence: { id: string };
        }>;
      };
      expect(payload.candidate.human_id).toBe("human-george");
      const cluster = payload.clusters.find((candidate) =>
        candidate.cluster_id.endsWith(":1"),
      )!;
      return Promise.resolve({
        text: JSON.stringify({
          mapping: {
            cluster_id: cluster.cluster_id,
            confidence: 0.98,
            evidence_id: cluster.evidence.id,
          },
        }),
      });
    });

    const updates = await inferAutomaticSpeakerAssignments({
      generatedSummary: "George Hotz criticized OpenAI's AI safety marketing.",
      model: {} as LanguageModel,
      snapshot: createSnapshot(),
      signal: new AbortController().signal,
    });

    expect(mocks.generateText).toHaveBeenCalledOnce();
    expect(automaticHumanIds(updates[0]!)).toEqual([
      "human-lex",
      "human-george",
    ]);
  });

  it("matches unique given names in generated summaries", async () => {
    mockDirectCandidateMatches();

    const updates = await inferAutomaticSpeakerAssignments({
      generatedSummary:
        "Lex mentions his conversation with Mark Zuckerberg. George strongly endorses open source AI.",
      model: {} as LanguageModel,
      snapshot: createSnapshot(),
      signal: new AbortController().signal,
    });

    expect(mocks.generateText).toHaveBeenCalledTimes(2);
    expect(automaticHumanIds(updates[0]!)).toEqual([
      "human-lex",
      "human-george",
    ]);
  });

  it("does not use a given name shared by multiple participants", async () => {
    const snapshot = createSnapshot();
    snapshot.participants[0]!.name = "Alex Smith";
    snapshot.participants[1]!.name = "Alex Jones";

    await expect(
      inferAutomaticSpeakerAssignments({
        generatedSummary: "Alex discussed open source AI.",
        model: {} as LanguageModel,
        snapshot,
        signal: new AbortController().signal,
      }),
    ).resolves.toEqual([]);
    expect(mocks.generateText).not.toHaveBeenCalled();
  });

  it.each([
    ["Will Smith", "will"],
    ["May Jones", "may"],
  ])(
    "does not treat common words as given-name evidence for %s",
    async (participantName, commonWord) => {
      const snapshot = createSnapshot();
      snapshot.participants[0]!.name = participantName;
      mocks.generateText.mockResolvedValue({
        text: JSON.stringify({ mapping: null }),
      });

      await inferAutomaticSpeakerAssignments({
        generatedSummary: `George Hotz ${commonWord} discuss open source AI.`,
        model: {} as LanguageModel,
        snapshot,
        signal: new AbortController().signal,
      });

      expect(mocks.generateText).toHaveBeenCalledOnce();
      const prompt = JSON.parse(mocks.generateText.mock.calls[0]![0].prompt);
      expect(prompt.candidate.human_id).toBe("human-george");
    },
  );

  it("does not treat a third-party full name as given-name evidence", async () => {
    const snapshot = createSnapshot();
    snapshot.participants[0]!.name = "Mark Smith";
    mocks.generateText.mockResolvedValue({
      text: JSON.stringify({ mapping: null }),
    });

    await inferAutomaticSpeakerAssignments({
      generatedSummary: "George Hotz discussed Mark Zuckerberg.",
      model: {} as LanguageModel,
      snapshot,
      signal: new AbortController().signal,
    });

    expect(mocks.generateText).toHaveBeenCalledOnce();
    const prompt = JSON.parse(mocks.generateText.mock.calls[0]![0].prompt);
    expect(prompt.candidate.human_id).toBe("human-george");
  });

  it("does not treat a third-party surname as given-name evidence", async () => {
    const snapshot = createSnapshot();
    snapshot.participants[0]!.name = "Jane Doe";
    mocks.generateText.mockResolvedValue({
      text: JSON.stringify({ mapping: null }),
    });

    await inferAutomaticSpeakerAssignments({
      generatedSummary: "George Hotz discussed Mary Jane.",
      model: {} as LanguageModel,
      snapshot,
      signal: new AbortController().signal,
    });

    expect(mocks.generateText).toHaveBeenCalledOnce();
    const prompt = JSON.parse(mocks.generateText.mock.calls[0]![0].prompt);
    expect(prompt.candidate.human_id).toBe("human-george");
  });

  it.each(["As", "When", "While"])(
    "matches a standalone given name after %s",
    async (prefix) => {
      const snapshot = createSnapshot();
      snapshot.participants[0]!.name = "Jane Doe";
      mocks.generateText.mockResolvedValue({
        text: JSON.stringify({ mapping: null }),
      });

      await inferAutomaticSpeakerAssignments({
        generatedSummary: `${prefix} Jane discussed open source AI.`,
        model: {} as LanguageModel,
        snapshot,
        signal: new AbortController().signal,
      });

      expect(mocks.generateText).toHaveBeenCalledOnce();
      const prompt = JSON.parse(mocks.generateText.mock.calls[0]![0].prompt);
      expect(prompt.candidate.human_id).toBe("human-lex");
    },
  );

  it("keeps standalone names distinct from joined given names", async () => {
    const snapshot = createSnapshot();
    snapshot.participants[0]!.name = "Mary-Jane Smith";
    snapshot.participants[1]!.name = "Jane Doe";
    mocks.generateText.mockResolvedValue({
      text: JSON.stringify({ mapping: null }),
    });

    await inferAutomaticSpeakerAssignments({
      generatedSummary: "Jane discussed open source AI.",
      model: {} as LanguageModel,
      snapshot,
      signal: new AbortController().signal,
    });

    expect(mocks.generateText).toHaveBeenCalledOnce();
    const prompt = JSON.parse(mocks.generateText.mock.calls[0]![0].prompt);
    expect(prompt.candidate.human_id).toBe("human-george");
  });

  it.each(["Mary-Jane Smith", "Mary'Jane Smith"])(
    "uses the full joined given name as evidence in %s",
    async (joinedName) => {
      const snapshot = createSnapshot();
      snapshot.participants[0]!.name = joinedName;
      snapshot.participants[1]!.name = "Jane Doe";
      mocks.generateText.mockResolvedValue({
        text: JSON.stringify({ mapping: null }),
      });

      await inferAutomaticSpeakerAssignments({
        generatedSummary: `${joinedName.split(" ")[0]} discussed open source AI.`,
        model: {} as LanguageModel,
        snapshot,
        signal: new AbortController().signal,
      });

      expect(mocks.generateText).toHaveBeenCalledOnce();
      const prompt = JSON.parse(mocks.generateText.mock.calls[0]![0].prompt);
      expect(prompt.candidate.human_id).toBe("human-lex");
    },
  );

  it("does not shorten a joined given name to its first token", async () => {
    const snapshot = createSnapshot();
    snapshot.participants[0]!.name = "Mary-Jane Smith";

    await expect(
      inferAutomaticSpeakerAssignments({
        generatedSummary: "Mary discussed open source AI.",
        model: {} as LanguageModel,
        snapshot,
        signal: new AbortController().signal,
      }),
    ).resolves.toEqual([]);
    expect(mocks.generateText).not.toHaveBeenCalled();
  });

  it("excludes shared given names that refer to another participant", async () => {
    const snapshot = createSnapshot();
    snapshot.participants[0]!.name = "Alex Smith";
    snapshot.participants[1]!.name = "Alex Jones";
    mocks.generateText.mockResolvedValue({
      text: JSON.stringify({ mapping: null }),
    });

    await inferAutomaticSpeakerAssignments({
      generatedSummary:
        "Alex Smith supported open source AI, and Alex disagreed with the approach.",
      model: {} as LanguageModel,
      snapshot,
      signal: new AbortController().signal,
    });

    expect(mocks.generateText).toHaveBeenCalledOnce();
    const prompt = JSON.parse(mocks.generateText.mock.calls[0]![0].prompt);
    expect(prompt.candidate).toMatchObject({
      human_id: "human-lex",
      summary_mentions: [
        {
          quote: "Alex Smith supported open source AI",
        },
      ],
    });
  });

  it("uses an exclusive clause when a summary sentence names both candidates", async () => {
    mocks.generateText.mockImplementation(({ prompt }: { prompt: string }) => {
      const payload = JSON.parse(prompt) as {
        candidate: { human_id: string };
        clusters: Array<{
          cluster_id: string;
          evidence: { id: string };
        }>;
      };
      expect(payload.candidate).toMatchObject({
        human_id: "human-george",
        summary_mentions: [
          {
            quote: "George Hotz argued open sourcing AI is safe",
          },
        ],
      });
      const cluster = payload.clusters.find((candidate) =>
        candidate.cluster_id.endsWith(":1"),
      )!;
      return Promise.resolve({
        text: JSON.stringify({
          mapping: {
            cluster_id: cluster.cluster_id,
            confidence: 0.98,
            evidence_id: cluster.evidence.id,
          },
        }),
      });
    });

    const updates = await inferAutomaticSpeakerAssignments({
      generatedSummary:
        "George Hotz argued open sourcing AI is safe, and referenced Lex Fridman's recent conversation with Mark Zuckerberg.",
      model: {} as LanguageModel,
      snapshot: createSnapshot(),
      signal: new AbortController().signal,
    });

    expect(mocks.generateText).toHaveBeenCalledOnce();
    expect(automaticHumanIds(updates[0]!)).toEqual([
      "human-lex",
      "human-george",
    ]);
  });

  it("selects exclusive summary evidence and the most relevant cluster quote", async () => {
    const snapshot = createSnapshot();
    snapshot.transcripts[0]!.words[2]!.text = ` ${"irrelevant filler ".repeat(24)}OpenAI used AI safety to hype its company`;
    mocks.generateText.mockResolvedValue({
      text: JSON.stringify({ mapping: null }),
    });

    await inferAutomaticSpeakerAssignments({
      generatedSummary:
        "George Hotz and Lex Fridman discussed open source. George Hotz criticized OpenAI for using AI safety as marketing.",
      model: {} as LanguageModel,
      snapshot,
      signal: new AbortController().signal,
    });

    expect(mocks.generateText).toHaveBeenCalledOnce();
    const prompt = JSON.parse(mocks.generateText.mock.calls[0]![0].prompt);
    expect(prompt.candidate).toMatchObject({
      human_id: "human-george",
      summary_mentions: [
        {
          quote:
            "George Hotz criticized OpenAI for using AI safety as marketing.",
        },
      ],
    });
    expect(
      prompt.clusters.find(
        (cluster: { cluster_id: string }) =>
          cluster.cluster_id === "transcript-1:1",
      ),
    ).toMatchObject({
      evidence: {
        id: "evidence-2",
        quote: expect.stringContaining("OpenAI used AI safety"),
      },
    });
  });

  it("attributes diarized mixed-capture batch transcripts", async () => {
    mockDirectCandidateMatches();

    const updates = await inferAutomaticSpeakerAssignments({
      generatedSummary:
        "Lex Fridman asked about Llama. George Hotz discussed open source.",
      model: {} as LanguageModel,
      snapshot: createSnapshot(2),
      signal: new AbortController().signal,
    });

    expect(automaticHumanIds(updates[0]!)).toEqual([
      "human-lex",
      "human-george",
    ]);
  });

  it.each([
    {
      name: "low confidence",
      mapping: {
        cluster_id: "transcript-1:1",
        confidence: 0.8,
        evidence_id: "evidence-1",
      },
    },
    {
      name: "unknown cluster",
      mapping: {
        cluster_id: "missing:1",
        confidence: 0.98,
        evidence_id: "evidence-1",
      },
    },
    {
      name: "unsupported evidence",
      mapping: {
        cluster_id: "transcript-1:1",
        confidence: 0.98,
        evidence_id: "not-supplied",
      },
    },
  ])("rejects a $name candidate match", async ({ mapping }) => {
    mocks.generateText.mockResolvedValue({
      text: JSON.stringify({ mapping }),
    });

    await expect(
      inferAutomaticSpeakerAssignments({
        generatedSummary: "George Hotz discussed open source.",
        model: {} as LanguageModel,
        snapshot: createSnapshot(),
        signal: new AbortController().signal,
      }),
    ).resolves.toEqual([]);
  });

  it("fails closed when candidate matches collide on one cluster", async () => {
    mocks.generateText.mockImplementation(({ prompt }: { prompt: string }) => {
      const payload = JSON.parse(prompt) as {
        clusters: Array<{
          cluster_id: string;
          evidence: { id: string };
        }>;
      };
      const cluster = payload.clusters.find((candidate) =>
        candidate.cluster_id.endsWith(":1"),
      )!;
      return Promise.resolve({
        text: JSON.stringify({
          mapping: {
            cluster_id: cluster.cluster_id,
            confidence: 0.98,
            evidence_id: cluster.evidence.id,
          },
        }),
      });
    });

    await expect(
      inferAutomaticSpeakerAssignments({
        generatedSummary:
          "Lex Fridman asked about Llama. George Hotz discussed open source.",
        model: {} as LanguageModel,
        snapshot: createSnapshot(),
        signal: new AbortController().signal,
      }),
    ).resolves.toEqual([]);
  });

  it("attributes recurring participants independently in each transcript", async () => {
    const snapshot = createSnapshot();
    const source = snapshot.transcripts[0]!;
    const wordIds = new Map(
      source.words.map((word) => [word.id, `second-${word.id}`]),
    );
    const secondSpeakerHints = source.speaker_hints.map((hint) => ({
      ...hint,
      id: `second-${hint.id}`,
      word_id:
        typeof hint.word_id === "string"
          ? (wordIds.get(hint.word_id) ?? hint.word_id)
          : hint.word_id,
    }));
    snapshot.transcripts.push({
      ...source,
      id: "transcript-2",
      wordsJson: "second words",
      speakerHintsJson: JSON.stringify(secondSpeakerHints),
      words: source.words.map((word) => ({
        ...word,
        id: wordIds.get(word.id)!,
      })),
      speaker_hints: secondSpeakerHints,
    });
    mockDirectCandidateMatches();

    const updates = await inferAutomaticSpeakerAssignments({
      generatedSummary:
        "Lex Fridman asked about Llama. George Hotz discussed open source.",
      model: {} as LanguageModel,
      snapshot,
      signal: new AbortController().signal,
    });

    expect(mocks.generateText).toHaveBeenCalledTimes(4);
    expect(updates.map((update) => update.id)).toEqual([
      "transcript-1",
      "transcript-2",
    ]);
    for (const update of updates) {
      expect(automaticHumanIds(update)).toEqual(["human-lex", "human-george"]);
    }
  });

  it("does not ask the model to overwrite an existing speaker assignment", async () => {
    const snapshot = createSnapshot();
    snapshot.transcripts[0]!.speaker_hints.push({
      id: "lex-1:user_speaker_assignment",
      word_id: "lex-1",
      type: "user_speaker_assignment",
      value: JSON.stringify({ human_id: "human-lex" }),
    });

    await expect(
      inferAutomaticSpeakerAssignments({
        generatedSummary:
          "Lex Fridman asked about Llama. George Hotz said Zuckerberg is a good guy.",
        model: {} as LanguageModel,
        snapshot,
        signal: new AbortController().signal,
      }),
    ).resolves.toEqual([]);
    expect(mocks.generateText).not.toHaveBeenCalled();
  });
});
