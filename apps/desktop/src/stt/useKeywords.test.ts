import { describe, expect, it } from "vitest";

import { extractKeywordsFromMarkdown } from "./useKeywords";

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
