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

const suggestionSchema = z.object({
  issues: z.array(
    z.object({
      line: z.number().describe("Line number in the content (1-indexed)"),
      original: z.string().describe("The original text that needs correction"),
      suggestion: z.string().describe("The corrected text"),
      reason: z
        .string()
        .describe("Brief explanation of why this change is needed"),
      category: z.enum([
        "em-dash",
        "punctuation-placement",
        "grammar",
        "spelling",
        "clarity",
        "other",
      ]),
    }),
  ),
  summary: z
    .string()
    .describe("Brief overall assessment of the article quality"),
});

async function checkGrammar(filename, content, contentWithLineNumbers) {
  const { object } = await generateObject({
    model: anthropic("claude-haiku-4-5"),
    schema: suggestionSchema,
    prompt: `You are a professional editor reviewing a blog article. Check the content for issues and provide specific, actionable suggestions.

## Style Rules (MUST flag these):

1. **Em dashes (—)**: Flag ALL em dashes. They should be replaced with regular dashes (-) or rewritten.
   - Example: "The tool—which is free—works great" → "The tool - which is free - works great" or rewrite the sentence

2. **Punctuation placement with quotes**: Periods and commas should go OUTSIDE quotation marks (British style).
   - Wrong: "lorem."
   - Correct: "lorem".
   - Wrong: "hello," she said
   - Correct: "hello", she said

## Also check for:
- Grammar and spelling errors
- Awkward phrasing or unclear sentences
- Other punctuation issues
- Consistency in tone and style

## Instructions:
- Provide the exact line number where each issue occurs
- Give the exact original text and the corrected version
- Be concise in explanations
- Only flag actual issues, not stylistic preferences (except for the rules above)

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
      "grammar-check-results.md",
      "## Grammar Check Results\n\nNo article files were changed in this PR.",
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

    const titleMatch =
      frontmatter.match(/display_title:\s*["']?(.+?)["']?\s*$/m) ||
      frontmatter.match(/meta_title:\s*["']?(.+?)["']?\s*$/m);
    const title = titleMatch ? titleMatch[1] : path.basename(file, ".mdx");

    console.log(`Checking: ${file}`);

    try {
      const contentWithLineNumbers = addLineNumbers(articleContent);
      const feedback = await checkGrammar(
        file,
        articleContent,
        contentWithLineNumbers,
      );

      results.push({
        file,
        title,
        feedback,
        frontmatterLines,
        contentLines: articleContent.split("\n"),
      });
    } catch (error) {
      results.push({
        file,
        title,
        feedback: null,
        error: error.message,
      });
    }
  }

  let markdown = "## Grammar Check Results\n\n";
  markdown += `Reviewed ${results.length} article${results.length === 1 ? "" : "s"}.\n\n`;

  for (const result of results) {
    markdown += `### ${result.title}\n`;
    markdown += `📄 \`${result.file}\`\n\n`;

    if (result.error) {
      markdown += `⚠️ Error: ${result.error}\n\n`;
    } else if (result.feedback.issues.length === 0) {
      markdown += `✅ No issues found!\n\n`;
      markdown += `${result.feedback.summary}\n\n`;
    } else {
      markdown += `${result.feedback.summary}\n\n`;
      markdown += `Found **${result.feedback.issues.length}** issue${result.feedback.issues.length === 1 ? "" : "s"}:\n\n`;

      const issuesByCategory = {};
      for (const issue of result.feedback.issues) {
        if (!issuesByCategory[issue.category]) {
          issuesByCategory[issue.category] = [];
        }
        issuesByCategory[issue.category].push(issue);
      }

      const categoryLabels = {
        "em-dash": "🔸 Em Dashes",
        "punctuation-placement": "🔹 Punctuation Placement",
        grammar: "📝 Grammar",
        spelling: "🔤 Spelling",
        clarity: "💡 Clarity",
        other: "📋 Other",
      };

      for (const [category, issues] of Object.entries(issuesByCategory)) {
        markdown += `#### ${categoryLabels[category] || category}\n\n`;

        for (const issue of issues) {
          const actualLine = issue.line + result.frontmatterLines;
          markdown += `**Line ${actualLine}**\n`;
          markdown += `> ${issue.original}\n\n`;
          markdown += `${issue.reason}\n\n`;

          markdown += `<details>\n<summary>📋 Suggested fix (click to expand)</summary>\n\n`;
          markdown += `\`\`\`suggestion\n${issue.suggestion}\n\`\`\`\n\n`;
          markdown += `</details>\n\n`;
        }
      }
    }

    markdown += "---\n\n";
  }

  markdown += "\n*Powered by Claude Haiku 4.5*";

  fs.writeFileSync("grammar-check-results.md", markdown);
  console.log(
    "Grammar check complete. Results written to grammar-check-results.md",
  );
}

main().catch((error) => {
  console.error("Grammar check failed:", error);
  fs.writeFileSync(
    "grammar-check-results.md",
    `## Grammar Check Results\n\n⚠️ Grammar check failed: ${error.message}`,
  );
  process.exit(1);
});
