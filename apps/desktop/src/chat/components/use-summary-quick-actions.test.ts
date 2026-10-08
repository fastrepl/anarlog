import { describe, expect, it } from "vitest";

import { canEditSummary } from "./use-summary-quick-actions";

const doc = (...content: unknown[]) => JSON.stringify({ type: "doc", content });
const paragraph = (text: string) => ({
  type: "paragraph",
  content: [{ type: "text", text }],
});
const titleHeading = (text: string) => ({
  type: "heading",
  attrs: { level: 1 },
  content: [{ type: "text", text }],
});

describe("canEditSummary", () => {
  it("enables rewrites when the summary chat would edit has content", () => {
    const notes = [
      { id: "summary", templateId: "", content: doc(paragraph("Decisions")) },
    ];
    expect(canEditSummary(notes, undefined, "Standup")).toBe(true);
    expect(canEditSummary(notes, "summary", "Standup")).toBe(true);
  });

  it("follows the open summary even when another summary has content", () => {
    const notes = [
      { id: "summary", templateId: "", content: doc(paragraph("Decisions")) },
      {
        id: "template",
        templateId: "tpl",
        content: doc({ type: "paragraph" }),
      },
    ];
    expect(canEditSummary(notes, "template", "Standup")).toBe(false);
  });

  it("treats a summary holding only the session title as empty", () => {
    const notes = [
      { id: "summary", templateId: "", content: doc(titleHeading("Standup")) },
    ];
    expect(canEditSummary(notes, undefined, "Standup")).toBe(false);
  });
});
