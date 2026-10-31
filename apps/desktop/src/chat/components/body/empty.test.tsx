import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("~/chat/hooks/use-chat-appearance", () => ({
  useChatAppearance: () => ({
    isDarkAppearance: false,
  }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: () => vi.fn(),
}));

const config = vi.hoisted(() => ({ customPrompts: "[]" }));

vi.mock("~/shared/config", () => ({
  useConfigValue: (key: string) =>
    key === "chat_custom_prompts" ? config.customPrompts : undefined,
}));

vi.mock("~/settings/queries", () => ({
  updateSettingValue: vi.fn(),
}));

import { ChatBodyEmpty } from "./empty";

describe("ChatBodyEmpty", () => {
  beforeEach(() => {
    cleanup();
    config.customPrompts = "[]";
  });

  it("offers different quick actions for notes, live meetings, and no context", () => {
    const prompts: string[] = [];
    for (const props of [
      { hasContext: true },
      { hasContext: true, isLiveMeeting: true },
      { hasContext: false },
    ]) {
      const onSendMessage = vi.fn();
      const view = render(
        <ChatBodyEmpty {...props} onSendMessage={onSendMessage} />,
      );
      fireEvent.click(screen.getAllByRole("button")[0]);
      const [prompt, parts] = onSendMessage.mock.calls[0];
      expect(prompt.trim().length).toBeGreaterThan(0);
      expect(parts).toEqual([{ type: "text", text: prompt }]);
      prompts.push(prompt);
      view.unmount();
    }
    expect(new Set(prompts).size).toBe(prompts.length);
  });

  it("offers shorter and longer rewrites only when the note has a summary", () => {
    const onSendMessage = vi.fn();
    const view = render(
      <ChatBodyEmpty hasContext hasSummary onSendMessage={onSendMessage} />,
    );
    fireEvent.click(screen.getByText("Make the summary shorter."));
    fireEvent.click(screen.getByText("Make the summary longer."));
    const [shorter, longer] = onSendMessage.mock.calls.map(
      ([prompt]) => prompt,
    );
    expect(shorter.trim()).not.toBe("");
    expect(longer.trim()).not.toBe("");
    expect(shorter).not.toEqual(longer);
    view.unmount();

    render(<ChatBodyEmpty hasContext onSendMessage={vi.fn()} />);
    expect(screen.queryByText("Make the summary shorter.")).toBeNull();
  });

  it("shows saved prompts only where they apply and sends them by name with the prompt kept out of the visible text", () => {
    config.customPrompts = JSON.stringify([
      {
        id: "live",
        title: "What should I say?",
        prompt: "Suggest my next line.",
        availability: "recording",
      },
      {
        id: "after",
        title: "Write a PRD",
        prompt: "Draft a PRD from this meeting.",
        availability: "not_recording",
      },
      {
        id: "any",
        title: "List risks",
        prompt: "List the risks raised so far.",
        availability: "always",
      },
    ]);
    const onSendMessage = vi.fn();
    const live = render(
      <ChatBodyEmpty hasContext isLiveMeeting onSendMessage={onSendMessage} />,
    );
    expect(screen.queryByText("Write a PRD")).toBeNull();
    expect(screen.getByText("List risks")).toBeTruthy();
    fireEvent.click(screen.getByText("What should I say?"));
    expect(onSendMessage).toHaveBeenCalledWith("What should I say?", [
      {
        type: "data-chat-prompt",
        data: {
          id: "live",
          title: "What should I say?",
          prompt: "Suggest my next line.",
        },
      },
    ]);
    live.unmount();

    render(<ChatBodyEmpty hasContext onSendMessage={vi.fn()} />);
    expect(screen.queryByText("What should I say?")).toBeNull();
    expect(screen.getByText("Write a PRD")).toBeTruthy();
    expect(screen.getByText("List risks")).toBeTruthy();
  });
});
