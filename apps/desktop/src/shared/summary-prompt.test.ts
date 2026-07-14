import { describe, expect, it } from "vitest";

import {
  DEFAULT_SUMMARY_PROMPT,
  getTokenAwareSummaryPrompt,
  hasSummaryTemplateToken,
  isDefaultSummaryPrompt,
  renderSummaryPrompt,
} from "./summary-prompt";

const template = {
  title: "Standup",
  description: "Daily team sync",
  sections: [
    { title: "Updates", description: null },
    { title: "Blockers", description: "Call out owners" },
  ],
};

describe("summary prompt", () => {
  it("recognizes the template token with flexible whitespace", () => {
    expect(hasSummaryTemplateToken("Before {{template}} after")).toBe(true);
    expect(hasSummaryTemplateToken("Before {{  template  }} after")).toBe(true);
    expect(hasSummaryTemplateToken("No template token")).toBe(false);
  });

  it("identifies the built-in prompt", () => {
    expect(isDefaultSummaryPrompt(DEFAULT_SUMMARY_PROMPT)).toBe(true);
    expect(isDefaultSummaryPrompt(`\n${DEFAULT_SUMMARY_PROMPT}\n`)).toBe(true);
    expect(isDefaultSummaryPrompt("Write a short summary")).toBe(false);
  });

  it("adds the template token to legacy custom instructions", () => {
    expect(getTokenAwareSummaryPrompt("Keep it brief", false)).toBe(
      "Keep it brief\n\n{{ template }}",
    );
  });

  it("preserves an intentional missing token after migration", () => {
    expect(getTokenAwareSummaryPrompt("Keep it brief", true)).toBe(
      "Keep it brief",
    );
  });

  it("injects the selected template at every template token", () => {
    const rendered = renderSummaryPrompt(
      "Use this:\n{{ template }}\nAgain:\n{{template}}",
      template,
    );

    expect(rendered).not.toContain("{{ template }}");
    expect(rendered.match(/# Summary Template/g)).toHaveLength(2);
    expect(rendered).toContain("Name: Standup");
    expect(rendered).toContain("Description: Daily team sync");
    expect(rendered).toContain("1. Updates");
    expect(rendered).toContain("2. Blockers - Call out owners");
  });

  it("leaves prompts without a template token unchanged", () => {
    expect(renderSummaryPrompt("Do not use headings.", template)).toBe(
      "Do not use headings.",
    );
  });
});
