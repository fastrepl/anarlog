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

const enhanced = (id: string) => ({ type: "enhanced" as const, id });

describe("canEditSummary", () => {
  const notes = [
    { id: "summary", templateId: "", content: doc(paragraph("Decisions")) },
    { id: "template", templateId: "tpl", content: doc({ type: "paragraph" }) },
    { id: "title-only", templateId: "", content: doc(titleHeading("Standup")) },
  ];

  it("offers rewrites only while a summary with content is shown", () => {
    expect(canEditSummary(notes, enhanced("summary"), false, "Standup")).toBe(
      true,
    );
    expect(canEditSummary(notes, { type: "raw" }, false, "Standup")).toBe(
      false,
    );
  });

  it("uses the summary a session tab shows by default", () => {
    expect(canEditSummary(notes, null, false, "Standup")).toBe(true);
    expect(canEditSummary(notes, null, true, "Standup")).toBe(false);
    const template = { ...notes[1], content: doc(paragraph("Agenda")) };
    expect(canEditSummary([template, notes[0]], null, false, "Standup")).toBe(
      false,
    );
    expect(canEditSummary([template], null, false, "Standup")).toBe(true);
  });

  it("follows the shown summary even when another summary has content", () => {
    expect(canEditSummary(notes, enhanced("template"), false, "Standup")).toBe(
      false,
    );
  });

  it("treats a summary holding only the session title as empty", () => {
    expect(
      canEditSummary(notes, enhanced("title-only"), false, "Standup"),
    ).toBe(false);
  });
});
