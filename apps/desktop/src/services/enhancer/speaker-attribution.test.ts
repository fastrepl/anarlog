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

describe("inferAutomaticSpeakerAssignments", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("creates guarded automatic hints from a complete high-confidence mapping", async () => {
    mocks.generateText.mockResolvedValue({
      text: `Here is the mapping:
\`\`\`json
${JSON.stringify({
  mappings: [
    {
      cluster_id: "transcript-1:0",
      human_id: "human-lex",
      confidence: 0.98,
      evidence_id: "evidence-1",
    },
    {
      cluster_id: "transcript-1:1",
      human_id: "human-george",
      confidence: 0.97,
      evidence: "evidence-1",
    },
  ],
})}
\`\`\``,
    });

    const updates = await inferAutomaticSpeakerAssignments({
      generatedSummary:
        "Lex Fridman asked about Llama. George Hotz said Zuckerberg is a good guy.",
      model: {} as LanguageModel,
      snapshot: createSnapshot(),
      signal: new AbortController().signal,
    });

    expect(updates).toEqual([
      expect.objectContaining({
        id: "transcript-1",
        currentWordsJson: "original words",
      }),
    ]);
    expect(
      JSON.parse(mocks.generateText.mock.calls[0]![0].prompt),
    ).toMatchObject({
      clusters: [
        {
          id: "transcript-1:0",
          evidence_candidates: [
            {
              id: "evidence-1",
              quote:
                "What do you think about open source Llama and the future of AI?",
            },
          ],
        },
        {
          id: "transcript-1:1",
          evidence_candidates: [
            {
              id: "evidence-1",
              quote:
                "Zuckerberg is a good guy and open source matters undoubtedly.",
            },
          ],
        },
      ],
    });
    expect(updates[0]!.expectedParticipantHumanIdsJson).toBe(
      '["human-george","human-lex"]',
    );
    const nextHints = JSON.parse(updates[0]!.nextSpeakerHintsJson);
    expect(
      nextHints.filter(
        (hint: { type: string }) =>
          hint.type === "automatic_speaker_assignment",
      ),
    ).toEqual([
      {
        id: "lex-1:automatic_speaker_assignment",
        word_id: "lex-1",
        type: "automatic_speaker_assignment",
        value: JSON.stringify({
          human_id: "human-lex",
          confidence: 0.98,
          source: "enhance",
        }),
      },
      {
        id: "george-1:automatic_speaker_assignment",
        word_id: "george-1",
        type: "automatic_speaker_assignment",
        value: JSON.stringify({
          human_id: "human-george",
          confidence: 0.97,
          source: "enhance",
        }),
      },
    ]);
  });

  it("attributes diarized mixed-capture batch transcripts", async () => {
    mocks.generateText.mockResolvedValue({
      text: JSON.stringify({
        mappings: [
          {
            cluster_id: "transcript-1:0",
            human_id: "human-lex",
            confidence: 0.98,
            evidence_id: "evidence-1",
          },
          {
            cluster_id: "transcript-1:1",
            human_id: "human-george",
            confidence: 0.97,
            evidence_id: "evidence-1",
          },
        ],
      }),
    });

    const updates = await inferAutomaticSpeakerAssignments({
      generatedSummary:
        "Lex Fridman asked about Llama. George Hotz discussed open source.",
      model: {} as LanguageModel,
      snapshot: createSnapshot(2),
      signal: new AbortController().signal,
    });

    expect(mocks.generateText).toHaveBeenCalledOnce();
    expect(
      JSON.parse(updates[0]!.nextSpeakerHintsJson).filter(
        (hint: { type: string }) =>
          hint.type === "automatic_speaker_assignment",
      ),
    ).toHaveLength(2);
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
    mocks.generateText.mockImplementation(({ prompt }: { prompt: string }) => {
      const clusters = JSON.parse(prompt).clusters as Array<{ id: string }>;
      return Promise.resolve({
        text: JSON.stringify({
          mappings: clusters.map((cluster, index) => ({
            cluster_id: cluster.id,
            human_id: index === 0 ? "human-lex" : "human-george",
            confidence: 0.98,
            evidence_id: "evidence-1",
          })),
        }),
      });
    });

    const updates = await inferAutomaticSpeakerAssignments({
      generatedSummary:
        "Lex Fridman asked about Llama. George Hotz discussed open source.",
      model: {} as LanguageModel,
      snapshot,
      signal: new AbortController().signal,
    });

    expect(mocks.generateText).toHaveBeenCalledTimes(2);
    expect(updates.map((update) => update.id)).toEqual([
      "transcript-1",
      "transcript-2",
    ]);
    for (const update of updates) {
      const automaticHints = JSON.parse(update.nextSpeakerHintsJson).filter(
        (hint: { type: string }) =>
          hint.type === "automatic_speaker_assignment",
      );
      expect(
        automaticHints.map(
          (hint: { value: string }) => JSON.parse(hint.value).human_id,
        ),
      ).toEqual(["human-lex", "human-george"]);
    }
  });

  it("rejects incomplete, duplicate, low-confidence, or unsupported mappings", async () => {
    const invalidMappings = [
      [
        {
          cluster_id: "transcript-1:0",
          human_id: "human-lex",
          confidence: 0.99,
          evidence_id: "evidence-1",
        },
      ],
      [
        {
          cluster_id: "transcript-1:0",
          human_id: "human-lex",
          confidence: 0.99,
          evidence_id: "evidence-1",
        },
        {
          cluster_id: "transcript-1:1",
          human_id: "human-lex",
          confidence: 0.99,
          evidence_id: "evidence-1",
        },
      ],
      [
        {
          cluster_id: "transcript-1:0",
          human_id: "human-lex",
          confidence: 1.1,
          evidence_id: "evidence-1",
        },
        {
          cluster_id: "transcript-1:1",
          human_id: "human-george",
          confidence: 0.99,
          evidence_id: "evidence-1",
        },
      ],
      [
        {
          cluster_id: "transcript-1:0",
          human_id: "human-lex",
          confidence: 0.8,
          evidence_id: "evidence-1",
        },
        {
          cluster_id: "transcript-1:1",
          human_id: "human-george",
          confidence: 0.99,
          evidence_id: "evidence-1",
        },
      ],
      [
        {
          cluster_id: "transcript-1:0",
          human_id: "human-lex",
          confidence: 0.99,
          evidence_id: "not-supplied",
        },
        {
          cluster_id: "transcript-1:1",
          human_id: "human-george",
          confidence: 0.99,
          evidence_id: "evidence-1",
        },
      ],
    ];

    for (const mappings of invalidMappings) {
      mocks.generateText.mockResolvedValueOnce({
        text: `\`\`\`json\n${JSON.stringify({ mappings })}\n\`\`\``,
      });
      await expect(
        inferAutomaticSpeakerAssignments({
          generatedSummary:
            "Lex Fridman asked about Llama. George Hotz said Zuckerberg is a good guy.",
          model: {} as LanguageModel,
          snapshot: createSnapshot(),
          signal: new AbortController().signal,
        }),
      ).resolves.toEqual([]);
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
