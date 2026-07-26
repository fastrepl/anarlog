import { anthropic } from "@ai-sdk/anthropic";
import { generateObject } from "ai";
import fs from "fs";
import path from "path";
import { z } from "zod";

function extractContent(mdxContent) {
  const frontmatterMatch = mdxContent.match(/^---\n([\s\S]*?)\n---\n/);
  if (frontmatterMatch) {
    return mdxContent.slice(frontmatterMatch[0].length);
  }
  return mdxContent;
}

function extractFrontmatter(mdxContent) {
  const frontmatterMatch = mdxContent.match(/^---\n([\s\S]*?)\n---\n/);
  if (frontmatterMatch) {
    return frontmatterMatch[1];
  }
  return "";
}

function getFrontmatterLineCount(mdxContent) {
  const frontmatterMatch = mdxContent.match(/^---\n([\s\S]*?)\n---\n/);
  if (frontmatterMatch) {
    return frontmatterMatch[0].split("\n").length - 1;
  }
  return 0;
}

function detectContentType(filePath) {
  if (filePath.includes("content/docs/")) return "docs";
  if (filePath.includes("content/handbook/")) return "handbook";
  if (filePath.includes("content/articles/")) return "articles";
  return "unknown";
}

function getTitle(frontmatter, filePath) {
  const titleMatch =
    frontmatter.match(/^title:\s*["']?(.+?)["']?\s*$/m) ||
    frontmatter.match(/display_title:\s*["']?(.+?)["']?\s*$/m) ||
    frontmatter.match(/meta_title:\s*["']?(.+?)["']?\s*$/m);
  return titleMatch ? titleMatch[1] : path.basename(filePath, ".mdx");
}

const auditSchema = z.object({
  issues: z.array(
    z.object({
      line: z.number().describe("Line number in the content (1-indexed)"),
      original: z
        .string()
        .describe("The exact original text that triggers the flag"),
      suggestion: z.string().describe("Rewritten text"),
      reason: z.string().describe("Brief explanation of the issue"),
      category: z.enum([
        "filler-phrase",
        "binary-contrast",
        "dramatic-fragmentation",
        "rhetorical-setup",
        "significance-inflation",
        "promotional-language",
        "ai-vocabulary",
        "copula-avoidance",
        "superficial-ing-phrase",
        "synonym-cycling",
        "generic-conclusion",
        "chatbot-artifact",
        "comma-splice",
        "dangling-modifier",
        "subject-verb-agreement",
        "passive-voice",
        "tense-inconsistency",
        "run-on",
        "punctuation",
        "grammar",
        "spelling",
        "clarity",
        "em-dash",
        "other",
      ]),
      severity: z
        .enum(["high", "medium", "low"])
        .describe(
          "high = clear issue, medium = likely problem, low = minor suggestion",
        ),
      pass: z
        .enum(["structure", "ai-patterns", "grammar"])
        .describe("Which audit pass found this issue"),
    }),
  ),
  scores: z.object({
    directness: z
      .number()
      .describe("1-10: Statements or announcements of statements?"),
    rhythm: z.number().describe("1-10: Varied or metronomic?"),
    trust: z.number().describe("1-10: Respects reader intelligence?"),
    authenticity: z.number().describe("1-10: Sounds like a person?"),
    density: z.number().describe("1-10: Anything cuttable without loss?"),
    grammar: z.number().describe("1-10: Clean or quietly breaking rules?"),
  }),
  signalDensity: z
    .number()
    .describe(
      "Ratio of sentences carrying new information to total sentences (0.0-1.0)",
    ),
  summary: z
    .string()
    .describe("Brief overall assessment of the content quality"),
});

// Based on https://github.com/ComputelessComputer/audit
const SYSTEM_PROMPT = `You are a writing auditor performing a comprehensive content review. Run a three-pass audit: structure/rhythm, AI patterns, and grammar.

# Pass 1: Structure & Rhythm

## Filler phrases - flag for removal
- "Here's the thing:" / "The truth is," / "Let me be clear"
- "Full stop." / "Let that sink in." / "This matters because"
- "At its core" / "In today's X" / "At the end of the day"
- "Hint:" / "Plot twist:" / "But that's another post"

## Structures to flag
- Binary contrasts: "Not X. But Y." - state Y directly
- Dramatic fragmentation: "[Noun]. That's it." - complete sentences
- Rhetorical setups: "What if [reframe]?" / "Think about it:" / "Here's what I mean:" - make the point directly
- Textbook inline examples: paired quoted terms as illustrations

## Rhythm checks
- Three consecutive sentences of same length? Flag.
- Every paragraph ends punchily? Flag.
- Three-item lists for rhetorical effect? Flag.
- Em-dash before a reveal? Use period or comma.
- Paragraphs starting with "So"? Start with content.

## Word patterns
- Absolute words (always, never, everyone, nobody) - false authority
- AI intensifiers (deeply, truly, fundamentally, inherently, simply, inevitably) - empty emphasis

# Pass 2: AI Patterns

## Significance inflation
Cut statements about how something "marks a pivotal moment," "underscores its importance," or "reflects broader trends." State what the thing does, not what it represents.
Flag: stands/serves as, testament, pivotal, underscores, highlights importance, evolving landscape, setting the stage for, indelible mark

## Promotional language
Neutral writing doesn't "boast," "showcase," or describe things as "vibrant," "breathtaking," or "nestled."
Flag: boasts, vibrant, rich (figurative), groundbreaking, renowned, must-visit, stunning, nestled, in the heart of

## Vague attributions
Replace "experts argue" and "observers note" with a specific source, or cut the claim.
Flag: Industry reports, Experts argue, Observers have cited, Some critics argue

## AI vocabulary overuse
High-frequency tells: Additionally, align with, crucial, delve, emphasizing, enduring, enhance, fostering, garner, highlight (verb), interplay, intricate, key (adjective), landscape (abstract), pivotal, showcase, tapestry (abstract), testament, underscore (verb), valuable, vibrant

## Copula avoidance
Replace "serves as," "stands as," "functions as," "boasts" with "is," "are," "has."

## Superficial -ing phrases
Tacked-on participial phrases fake depth. If the -ing phrase doesn't add a specific fact, cut it.
Flag: highlighting..., symbolizing..., reflecting..., contributing to..., fostering..., showcasing...

## Rule of three
Two items work. Three feels assembled. One is often enough.

## Negative parallelisms
"It's not just about X; it's Y" - just say Y.

## Synonym cycling
Pick one word for a subject. Don't rotate protagonist -> main character -> central figure -> hero.

## Generic conclusions
"The future looks bright" / "exciting times lie ahead" - end with a specific fact or observation.

## Chatbot artifacts
Remove: "I hope this helps," "Let me know if you'd like me to expand," "Great question!", "Certainly!"

# Pass 3: Grammar

Check for:
- Comma splices
- Dangling modifiers
- Subject-verb agreement
- Its/it's, their/there/they're, affect/effect
- Passive voice overuse
- Apostrophe misuse
- Tense consistency
- Run-on sentences
- Em dashes (should be regular dashes or rewritten)
- Punctuation placement with quotes (periods and commas outside quotation marks)

# Signal Density
Signal density = (sentences carrying new information or genuine insight) / (total sentences)
Target: >= 0.90

# Scoring
Rate 1-10 on each dimension:
- Directness: Statements or announcements of statements?
- Rhythm: Varied or metronomic?
- Trust: Respects reader intelligence?
- Authenticity: Sounds like a person, not an algorithm?
- Density: Anything cuttable without loss?
- Grammar: Clean or quietly breaking rules?

# Instructions
1. Run all three passes in order.
2. For each issue, provide exact line number, original text, suggested fix, and reason.
3. Be thorough but fair - only flag actual issues, not stylistic preferences (except for the rules above).
4. Score the text on all dimensions.
5. Calculate signal density.
6. Provide a brief overall assessment.`;

async function auditContent(contentType, contentWithLineNumbers) {
  const contextNote =
    contentType === "docs"
      ? "This is product documentation. Focus on clarity, accuracy, and directness. Technical precision matters more than voice."
      : contentType === "handbook"
        ? "This is a company handbook. The voice should be direct and opinionated but not promotional. Focus on clarity and actionability."
        : "This is a blog article. Full audit applies.";

  const { object } = await generateObject({
    model: anthropic("claude-haiku-4-5"),
    schema: auditSchema,
    system: SYSTEM_PROMPT,
    prompt: `Review the following ${contentType} content. ${contextNote}

Content with line numbers:
${contentWithLineNumbers}`,
  });

  return object;
}

function addLineNumbers(content) {
  return content
    .split("\n")
    .map((line, i) => `${i + 1}: ${line}`)
    .join("\n");
}

async function main() {
  const changedFiles =
    process.env.CHANGED_FILES?.trim().split(" ").filter(Boolean) || [];

  if (changedFiles.length === 0) {
    fs.writeFileSync(
      "content-audit-results.md",
      "## Content Audit Results\n\nNo content files were changed in this PR.",
    );
    return;
  }

  const results = [];

  for (const file of changedFiles) {
    if (!fs.existsSync(file)) {
      continue;
    }

    const fullContent = fs.readFileSync(file, "utf8");
    const articleContent = extractContent(fullContent);
    const frontmatter = extractFrontmatter(fullContent);
    const frontmatterLines = getFrontmatterLineCount(fullContent);
    const contentType = detectContentType(file);
    const title = getTitle(frontmatter, file);

    console.log(`Auditing (${contentType}): ${file}`);

    try {
      const contentWithLineNumbers = addLineNumbers(articleContent);
      const feedback = await auditContent(contentType, contentWithLineNumbers);

      const totalScore =
        feedback.scores.directness +
        feedback.scores.rhythm +
        feedback.scores.trust +
        feedback.scores.authenticity +
        feedback.scores.density +
        feedback.scores.grammar;

      results.push({
        file,
        title,
        contentType,
        feedback,
        frontmatterLines,
        totalScore,
      });
    } catch (error) {
      results.push({
        file,
        title,
        contentType,
        feedback: null,
        error: error.message,
      });
    }
  }

  let markdown = "## Content Audit Results\n\n";
  markdown += `Reviewed ${results.length} file${results.length === 1 ? "" : "s"} using [audit](https://github.com/ComputelessComputer/audit) methodology.\n\n`;

  for (const result of results) {
    const typeLabel =
      result.contentType === "docs"
        ? "Documentation"
        : result.contentType === "handbook"
          ? "Handbook"
          : "Article";

    markdown += `### ${result.title}\n`;
    markdown += `\`${result.file}\` (${typeLabel})\n\n`;

    if (result.error) {
      markdown += `Error: ${result.error}\n\n`;
    } else {
      const { feedback, totalScore } = result;
      const passIcon = totalScore >= 42 ? "PASS" : "NEEDS REVISION";
      const densityIcon =
        feedback.signalDensity >= 0.9 ? "PASS" : "NEEDS REVISION";

      markdown += `**Score: ${totalScore}/60** (${passIcon}) | **Signal Density: ${(feedback.signalDensity * 100).toFixed(0)}%** (${densityIcon})\n\n`;
      markdown += `| Dimension | Score |\n|-----------|-------|\n`;
      markdown += `| Directness | ${feedback.scores.directness}/10 |\n`;
      markdown += `| Rhythm | ${feedback.scores.rhythm}/10 |\n`;
      markdown += `| Trust | ${feedback.scores.trust}/10 |\n`;
      markdown += `| Authenticity | ${feedback.scores.authenticity}/10 |\n`;
      markdown += `| Density | ${feedback.scores.density}/10 |\n`;
      markdown += `| Grammar | ${feedback.scores.grammar}/10 |\n\n`;

      markdown += `${feedback.summary}\n\n`;

      if (feedback.issues.length === 0) {
        markdown += `No issues found.\n\n`;
      } else {
        const highCount = feedback.issues.filter(
          (i) => i.severity === "high",
        ).length;
        const medCount = feedback.issues.filter(
          (i) => i.severity === "medium",
        ).length;
        const lowCount = feedback.issues.filter(
          (i) => i.severity === "low",
        ).length;

        markdown += `Found **${feedback.issues.length}** issue${feedback.issues.length === 1 ? "" : "s"}`;
        markdown += ` (${highCount} high, ${medCount} medium, ${lowCount} low)\n\n`;

        const passList = ["structure", "ai-patterns", "grammar"];
        const passLabels = {
          structure: "Structure & Rhythm",
          "ai-patterns": "AI Patterns",
          grammar: "Grammar",
        };

        for (const pass of passList) {
          const issues = feedback.issues.filter((i) => i.pass === pass);
          if (issues.length === 0) continue;

          markdown += `#### ${passLabels[pass]}\n\n`;

          const severityOrder = ["high", "medium", "low"];
          for (const severity of severityOrder) {
            const sevIssues = issues.filter((i) => i.severity === severity);
            for (const issue of sevIssues) {
              const actualLine = issue.line + result.frontmatterLines;
              const sevIcon =
                severity === "high"
                  ? "HIGH"
                  : severity === "medium"
                    ? "MED"
                    : "LOW";
              markdown += `**Line ${actualLine}** [${sevIcon}] \`${issue.category}\`\n`;
              markdown += `> ${issue.original}\n\n`;
              markdown += `${issue.reason}\n\n`;
              markdown += `<details>\n<summary>Suggested fix</summary>\n\n`;
              markdown += `\`\`\`suggestion\n${issue.suggestion}\n\`\`\`\n\n`;
              markdown += `</details>\n\n`;
            }
          }
        }
      }
    }

    markdown += "---\n\n";
  }

  markdown +=
    "\n*Powered by Claude Haiku 4.5 with [audit](https://github.com/ComputelessComputer/audit) methodology*";

  fs.writeFileSync("content-audit-results.md", markdown);
  console.log(
    "Content audit complete. Results written to content-audit-results.md",
  );
}

main().catch((error) => {
  console.error("Content audit failed:", error);
  fs.writeFileSync(
    "content-audit-results.md",
    `## Content Audit Results\n\nContent audit failed: ${error.message}`,
  );
  process.exit(1);
});
