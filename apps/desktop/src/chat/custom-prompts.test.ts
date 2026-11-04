import { describe, expect, it, vi } from "vitest";

const settings = vi.hoisted(() => ({
  stored: undefined as string | undefined,
}));

vi.mock("~/settings/queries", () => ({
  updateSettingValue: async (
    _key: string,
    update: (current: string | undefined) => string,
  ) => {
    settings.stored = update(settings.stored);
    return settings.stored;
  },
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: vi.fn(),
}));

import {
  deleteCustomChatPrompt,
  parseCustomChatPrompts,
  saveCustomChatPrompt,
} from "./custom-prompts";

describe("custom chat prompts", () => {
  it("edits saved prompts in place instead of duplicating them and survives malformed rows", async () => {
    settings.stored = JSON.stringify([
      {
        id: "live",
        title: "What should I say?",
        prompt: "Suggest my next line.",
        availability: "recording",
      },
      { id: "untitled", title: " ", prompt: "Missing a name." },
      "not a prompt",
    ]);

    await saveCustomChatPrompt({
      id: "live",
      title: "What should I say next?",
      prompt: "Suggest my next line.",
      availability: "recording",
    });
    await saveCustomChatPrompt({
      id: "prd",
      title: "Write a PRD",
      prompt: "Draft a PRD from this meeting.",
      availability: "not_recording",
    });

    expect(parseCustomChatPrompts(settings.stored)).toEqual([
      {
        id: "live",
        title: "What should I say next?",
        prompt: "Suggest my next line.",
        availability: "recording",
      },
      {
        id: "prd",
        title: "Write a PRD",
        prompt: "Draft a PRD from this meeting.",
        availability: "not_recording",
      },
    ]);

    await deleteCustomChatPrompt("live");

    expect(
      parseCustomChatPrompts(settings.stored).map((prompt) => prompt.id),
    ).toEqual(["prd"]);
  });
});
