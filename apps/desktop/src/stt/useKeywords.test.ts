import { describe, expect, it } from "vitest";

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
  type KeywordStore,
} from "./useKeywords";

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
  it("builds keywords from the current session snapshot", () => {
    const cells = new Map([
      ["raw_md", "Discuss #Launch and production systems"],
      ["title", "Erebor sync"],
      [
        "event_json",
        JSON.stringify({
          title: "OpenWorld review",
          description: "Airborne Brothers follow-up",
          location: "Zoom",
        }),
      ],
    ]);
    const store = {
      getCell: (_tableId: "sessions", _rowId: string, cellId: string) =>
        cells.get(cellId),
    };

    expect(
      getSessionKeywords({
        store: store as unknown as KeywordStore,
        sessionId: "session-1",
        dictionaryTerms: ["Anarlog"],
      }),
    ).toEqual(expect.arrayContaining(["Anarlog", "Launch"]));
  });

  it("prioritizes mapped participants and attached event attendees", () => {
    const tables = {
      sessions: new Map([
        [
          "session-1",
          {
            raw_md: "Discuss #Launch and production systems",
            title: "Erebor sync",
            event_json: JSON.stringify({
              tracking_id: "tracking-1",
              calendar_id: "calendar-1",
              title: "OpenWorld review",
              description: "Airborne Brothers follow-up",
              location: "Zoom",
            }),
          },
        ],
      ]),
      mapping_session_participant: new Map([
        [
          "mapping-1",
          {
            session_id: "session-1",
            human_id: "human-1",
          },
        ],
        [
          "mapping-2",
          {
            session_id: "other-session",
            human_id: "human-2",
          },
        ],
        [
          "mapping-3",
          {
            session_id: "session-1",
            human_id: "human-3",
            source: "excluded",
          },
        ],
      ]),
      humans: new Map([
        ["human-1", { name: "Alice Kim" }],
        ["human-2", { name: "Bob Stone" }],
        ["human-3", { name: "Hidden Person" }],
      ]),
      events: new Map([
        [
          "event-1",
          {
            tracking_id_event: "tracking-1",
            calendar_id: "calendar-1",
            participants_json: JSON.stringify([
              { name: "Alice Kim", email: "alice@example.com" },
              { name: "Mina Park", email: "mina@example.com" },
              {
                name: "John Jeong",
                email: "john@example.com",
                is_current_user: true,
              },
            ]),
          },
        ],
      ]),
    };
    const store = {
      getCell: (tableId: keyof typeof tables, rowId: string, cellId: string) =>
        (tables[tableId].get(rowId) as Record<string, unknown> | undefined)?.[
          cellId
        ],
      forEachRow: (
        tableId: "mapping_session_participant" | "events",
        callback: (rowId: string, forEachCell: unknown) => void,
      ) => {
        for (const rowId of tables[tableId].keys()) {
          callback(rowId, undefined);
        }
      },
    };

    const result = getSessionKeywords({
      store: store as unknown as KeywordStore,
      sessionId: "session-1",
      dictionaryTerms: ["Anarlog"],
    });

    expect(result.slice(0, 3)).toEqual(["Alice Kim", "Mina Park", "Anarlog"]);
    expect(result).toEqual(expect.arrayContaining(["Launch"]));
    expect(result).not.toContain("Bob Stone");
    expect(result).not.toContain("Hidden Person");
    expect(result).not.toContain("John Jeong");
  });
});

describe("buildKeywords", () => {
  it("dedupes higher-priority hints and caps the result", () => {
    const result = buildKeywords({
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
