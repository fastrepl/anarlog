import type { EnhanceTemplate } from "@hypr/plugin-template";

export const SUMMARY_TEMPLATE_TOKEN = "{{ template }}";

export const DEFAULT_SUMMARY_PROMPT = `Use the selected summary template for the summary structure and section headings.

${SUMMARY_TEMPLATE_TOKEN}`;

const SUMMARY_TEMPLATE_TOKEN_PATTERN = /\{\{\s*template\s*\}\}/;
const SUMMARY_TEMPLATE_TOKEN_PATTERN_GLOBAL = /\{\{\s*template\s*\}\}/g;

export function hasSummaryTemplateToken(prompt: string): boolean {
  return SUMMARY_TEMPLATE_TOKEN_PATTERN.test(prompt);
}

export function isDefaultSummaryPrompt(prompt: string): boolean {
  return prompt.trim() === DEFAULT_SUMMARY_PROMPT;
}

export function getTokenAwareSummaryPrompt(
  prompt: string,
  tokenAware: boolean,
): string {
  const trimmed = prompt.trim();
  if (!trimmed) {
    return DEFAULT_SUMMARY_PROMPT;
  }
  if (tokenAware || hasSummaryTemplateToken(trimmed)) {
    return trimmed;
  }
  return `${trimmed}\n\n${SUMMARY_TEMPLATE_TOKEN}`;
}

export function renderSummaryPrompt(
  prompt: string,
  template: EnhanceTemplate | null,
): string {
  return prompt.replace(SUMMARY_TEMPLATE_TOKEN_PATTERN_GLOBAL, () =>
    formatSummaryTemplate(template),
  );
}

function formatSummaryTemplate(template: EnhanceTemplate | null): string {
  if (!template) {
    return `# Instructions

1. Analyze the content and decide the sections to use.
2. Generate a well-formatted markdown summary.`;
  }

  const lines = ["# Summary Template"];

  if (template.title.trim()) {
    lines.push("", `Name: ${template.title.trim()}`);
  }
  if (template.description?.trim()) {
    lines.push(`Description: ${template.description.trim()}`);
  }

  lines.push("", "Sections:");
  template.sections.forEach((section, index) => {
    const description = section.description?.trim();
    lines.push(
      `${index + 1}. ${section.title.trim()}${description ? ` - ${description}` : ""}`,
    );
  });

  return lines.join("\n");
}
