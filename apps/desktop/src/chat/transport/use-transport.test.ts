import { describe, expect, it } from "vitest";

import { appendFileContextToolGuidance } from "./use-transport";

describe("chat transport prompt guidance", () => {
  it("tells chat to search meeting notes when no meeting note context is attached", () => {
    const prompt = appendFileContextToolGuidance("Base prompt");

    expect(prompt).toContain("Base prompt");
    expect(prompt).toContain("When no meeting note context is attached");
    expect(prompt).toContain("use search_sessions");
    expect(prompt).toContain(
      "Do not ask the user to open or share a meeting note",
    );
  });
});
