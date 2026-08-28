import { cleanup, render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const appearanceState = vi.hoisted(() => ({
  isDarkAppearance: true,
}));

vi.mock("~/chat/hooks/use-chat-appearance", () => ({
  useChatAppearance: () => ({
    isDarkAppearance: appearanceState.isDarkAppearance,
  }),
}));

import { MessageBubble } from "./shared";

describe("MessageBubble", () => {
  beforeEach(() => {
    cleanup();
    appearanceState.isDarkAppearance = true;
  });

  it("selects the dark assistant bubble appearance", () => {
    const { container } = render(
      <MessageBubble variant="assistant">Hello</MessageBubble>,
    );

    const bubble = container.firstChild as HTMLElement;

    expect(bubble.dataset.chatMessageAppearance).toBe("dark");
    expect(bubble.dataset.chatMessageVariant).toBe("assistant");
  });

  it("selects the user bubble variant", () => {
    const { container } = render(
      <MessageBubble variant="user">Hello</MessageBubble>,
    );

    const bubble = container.firstChild as HTMLElement;

    expect(bubble.dataset.chatMessageAppearance).toBe("dark");
    expect(bubble.dataset.chatMessageVariant).toBe("user");
  });
});
