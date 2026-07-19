import { beforeEach, describe, expect, it, vi } from "vitest";

const execute = vi.hoisted(() => vi.fn());

vi.mock("~/db", () => ({
  liveQueryClient: { execute },
  useLiveQuery: vi.fn(() => ({ data: undefined })),
}));

import {
  formatDictionaryTerms,
  normalizeKeywordList,
  parseDictionaryTermsText,
} from "./keywords";
import {
  buildKeywords,
  buildKeywordSourceText,
  extractKeywordsFromMarkdown,
  getSessionKeywords,
} from "./useKeywords";

beforeEach(() => {
  execute.mockReset();
  execute.mockResolvedValue([]);
});

describe("extractKeywordsFromMarkdown", () => {
  const cases: Array<{
    description: string;
    input: string;
    keywords: string[];
    keyphrases: string[];
  }> = [
    {
      description: "extracts hashtags from markdown",
      input: "This is #awesome and #cool stuff",
      keywords: ["awesome", "cool"],
      keyphrases: [],
    },
    {
      description: "handles unicode hashtags",
      input: "#日本語 #한글 #Español",
      keywords: ["日本語", "한글", "Español"],
      keyphrases: [],
    },
    {
      description: "excludes code blocks and inline code",
      input:
        "Use the `useState` hook\n```js\nconst value = 1;\n```\nKeep reading for keywords",
      keywords: ["hook", "keywords"],
      keyphrases: [],
    },
    {
      description: "extracts hashtags and keywords from markdown",
      input:
        "# Hello World\n\nThis is a #test with some #keywords and important content",
      keywords: ["test", "keywords", "World", "content"],
      keyphrases: [],
    },
    {
      description: "removes code blocks before processing",
      input:
        "Text before\n```js\nconst x = 1;\n```\nText after with important keywords",
      keywords: ["Text", "keywords"],
      keyphrases: [],
    },
    {
      description: "extracts keywords from natural language",
      input:
        "Artificial intelligence and machine learning are transforming technology",
      keywords: ["technology"],
      keyphrases: ["Artificial intelligence"],
    },
    {
      description: "extracts keyphrases",
      input: "I'm learning about machine learning and data science in depth",
      keywords: [],
      keyphrases: ["data science"],
    },
    {
      description: "handles complex real-world markdown",
      input: `
# Meeting Notes

We discussed machine learning and its applications in production systems.

\`\`\`python
def hello():
    print("hello")
\`\`\`

Key points:
- Review the #projectA deliverables
- Implement data science solutions
- Team collaboration is essential

Next steps: testing and validation of the algorithms
    `,
      keywords: ["projectA"],
      keyphrases: ["production systems"],
    },
    {
      description: "handles empty text gracefully",
      input: "",
      keywords: [],
      keyphrases: [],
    },
    {
      description: "filters single-character tokens",
      input: "a b c",
      keywords: [],
      keyphrases: [],
    },
    {
      description: "comma & space",
      input: "yujonglee,john, atila",
      keywords: ["yujonglee", "john", "atila"],
      keyphrases: [],
    },
  ];

  it.each(cases)("$description", ({ input, keywords, keyphrases }) => {
    const result = extractKeywordsFromMarkdown(input);
    expect(result.keywords).toEqual(expect.arrayContaining(keywords));
    expect(result.keyphrases).toEqual(expect.arrayContaining(keyphrases));
  });
});

describe("buildKeywordSourceText", () => {
  it("includes session note, title, and event metadata", () => {
    expect(
      buildKeywordSourceText({
        rawMd: "Discuss product launch",
        title: "Erebor sync",
        eventJson: JSON.stringify({
          title: "OpenWorld review",
          description: "Airborne Brothers follow-up",
          location: "Zoom",
        }),
      }),
    ).toBe(
      [
        "Discuss product launch",
        "Erebor sync",
        "OpenWorld review",
        "Airborne Brothers follow-up",
        "Zoom",
      ].join("\n"),
    );
  });
});

describe("getSessionKeywords", () => {
  it("builds keywords from the current session snapshot", async () => {
    execute.mockResolvedValue([
      {
        raw_md: "Discuss #Launch and production systems",
        title: "Erebor sync",
        event_json: JSON.stringify({
          title: "OpenWorld review",
          description: "Airborne Brothers follow-up",
          location: "Zoom",
        }),
        participant_names_json: "[]",
        event_participants_json: "[]",
      },
    ]);

    await expect(
      getSessionKeywords({
        sessionId: "session-1",
        dictionaryTerms: ["Anarlog"],
        allowCalendarDerivedHints: true,
      }),
    ).resolves.toEqual(expect.arrayContaining(["Anarlog", "Launch"]));
  });

  it("prioritizes mapped participants and attached event attendees", async () => {
    execute.mockResolvedValue([
      {
        raw_md: "Discuss #Launch and production systems",
        title: "Erebor sync",
        event_json: JSON.stringify({
          title: "OpenWorld review",
          description: "Airborne Brothers follow-up",
          location: "Zoom",
        }),
        participant_names_json: JSON.stringify(["Alice Kim"]),
        event_participants_json: JSON.stringify([
          { name: "Alice Kim", email: "alice@example.com" },
          { name: "Mina Park", email: "mina@example.com" },
          {
            name: "John Jeong",
            email: "john@example.com",
            is_current_user: true,
          },
        ]),
      },
    ]);

    const result = await getSessionKeywords({
      sessionId: "session-1",
      dictionaryTerms: ["Anarlog"],
      allowCalendarDerivedHints: true,
    });

    expect(result.slice(0, 3)).toEqual(["Alice Kim", "Mina Park", "Anarlog"]);
    expect(result).toEqual(expect.arrayContaining(["Launch"]));
    expect(result).not.toContain("John Jeong");
  });

  it("withholds calendar event and attendee hints from off-device STT", async () => {
    execute.mockResolvedValue([
      {
        raw_md: "Remember #UserMemo",
        title: "GoogleOnlyTitle",
        event_json: JSON.stringify({
          title: "GoogleOnlyEvent",
          description: "GoogleOnlyDescription",
          location: "GoogleOnlyLocation",
        }),
        participant_names_json: JSON.stringify(["GoogleOnlyParticipant"]),
        event_participants_json: JSON.stringify([
          { name: "GoogleOnlyAttendee" },
        ]),
      },
    ]);

    const result = await getSessionKeywords({
      sessionId: "session-google",
      dictionaryTerms: ["UserDictionary"],
      allowCalendarDerivedHints: false,
    });

    expect(result).toEqual(
      expect.arrayContaining(["UserMemo", "UserDictionary"]),
    );
    expect(result.join(" ")).not.toContain("GoogleOnly");
  });

  it("retains Google hints for explicitly on-device STT", async () => {
    execute.mockResolvedValue([
      {
        raw_md: "",
        title: "GoogleLocalTitle",
        event_json: "",
        participant_names_json: JSON.stringify(["Google Local Participant"]),
        event_participants_json: "[]",
      },
    ]);

    await expect(
      getSessionKeywords({
        sessionId: "session-google",
        dictionaryTerms: [],
        allowCalendarDerivedHints: true,
      }),
    ).resolves.toEqual(
      expect.arrayContaining(["Google Local Participant", "GoogleLocalTitle"]),
    );
  });

  it("returns dictionary-only hints when session provenance is unresolved", async () => {
    execute.mockResolvedValue([]);

    await expect(
      getSessionKeywords({
        sessionId: "missing-session",
        dictionaryTerms: ["UserDictionary"],
        allowCalendarDerivedHints: false,
      }),
    ).resolves.toEqual(["UserDictionary"]);
  });
});

describe("buildKeywords", () => {
  it("dedupes higher-priority hints and caps the result", () => {
    const result = buildKeywords({
      allowCalendarDerivedHints: true,
      rawMd: "",
      title: "",
      eventJson: "",
      sessionParticipantTerms: ["Alice Kim"],
      eventParticipantTerms: ["alice kim", "Mina Park"],
      dictionaryTerms: Array.from(
        { length: 60 },
        (_, index) => `Term ${index}`,
      ),
    });

    expect(result).toHaveLength(50);
    expect(result.slice(0, 4)).toEqual([
      "Alice Kim",
      "Mina Park",
      "Term 0",
      "Term 1",
    ]);
    expect(result).not.toContain("alice kim");
  });
});

describe("dictionary term helpers", () => {
  it("parses newline and comma separated terms", () => {
    expect(
      parseDictionaryTermsText("Anarlog\nFastConformer, Parakeet TDT"),
    ).toEqual(["Anarlog", "FastConformer", "Parakeet TDT"]);
  });

  it("normalizes duplicate terms while preserving first spelling", () => {
    expect(normalizeKeywordList(["Anarlog", " anarlog ", "Parakeet"])).toEqual([
      "Anarlog",
      "Parakeet",
    ]);
  });

  it("formats stored terms one per line", () => {
    expect(formatDictionaryTerms(["Anarlog", "Parakeet TDT"])).toBe(
      "Anarlog\nParakeet TDT",
    );
  });
});
