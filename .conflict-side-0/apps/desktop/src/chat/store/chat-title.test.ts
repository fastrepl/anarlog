import { generateText } from "ai";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  createFallbackChatTitle,
  generateChatTitle,
  normalizeGeneratedChatTitle,
} from "./chat-title";

vi.mock("ai", () => ({
  generateText: vi.fn(),
}));

describe("chat title", () => {
  beforeEach(() => {
    vi.mocked(generateText).mockReset();
  });

  it("creates an immediate fallback title from the initial request", () => {
    const title = createFallbackChatTitle(
      "  Please   summarize this request for tomorrow's roadmap planning and share action items  ",
    );

    expect(title).toBe("Please summarize this request for tomorrow's...");
    expect(title.length).toBeLessThanOrEqual(50);
  });

  it("falls back to a default title for blank requests", () => {
    expect(createFallbackChatTitle("   ")).toBe("New chat");
  });

  it("normalizes generated titles", () => {
    expect(
      normalizeGeneratedChatTitle('1. "Review onboarding fixes."\nExtra text'),
    ).toBe("Review onboarding fixes");
    expect(normalizeGeneratedChatTitle("")).toBeNull();
  });

  it("summarizes the initial request with the title model", async () => {
    vi.mocked(generateText).mockResolvedValue({
      text: '"Review onboarding fixes."',
    } as any);

    const model = {} as any;
    const title = await generateChatTitle({
      model,
      initialRequest: "Can you review the onboarding flow regressions?",
    });

    expect(title).toBe("Review onboarding fixes");
    expect(generateText).toHaveBeenCalledWith(
      expect.objectContaining({
        model,
        temperature: 0,
        maxOutputTokens: 32,
        prompt: expect.stringContaining(
          "Can you review the onboarding flow regressions?",
        ),
      }),
    );
  });

  it("does not call the model for blank requests", async () => {
    const title = await generateChatTitle({
      model: {} as any,
      initialRequest: "   ",
    });

    expect(title).toBeNull();
    expect(generateText).not.toHaveBeenCalled();
  });
});
