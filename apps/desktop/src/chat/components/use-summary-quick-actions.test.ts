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
  const notes = [
    { id: "summary", content: doc(paragraph("Decisions")) },
    { id: "template", content: doc({ type: "paragraph" }) },
    { id: "title-only", content: doc(titleHeading("Standup")) },
  ];

  it("offers rewrites only while a summary with content is open", () => {
    expect(canEditSummary(notes, "summary", "Standup")).toBe(true);
    expect(canEditSummary(notes, undefined, "Standup")).toBe(false);
  });

  it("follows the open summary even when another summary has content", () => {
    expect(canEditSummary(notes, "template", "Standup")).toBe(false);
  });

  it("treats a summary holding only the session title as empty", () => {
    expect(canEditSummary(notes, "title-only", "Standup")).toBe(false);
  });
});
