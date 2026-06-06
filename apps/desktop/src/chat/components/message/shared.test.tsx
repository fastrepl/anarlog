import { cleanup, render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const appearanceState = vi.hoisted(() => ({
  isDarkAppearance: true,
}));

vi.mock("~/chat/hooks/use-chat-appearance", () => ({
  useChatAppearance: () => ({
    isDarkAppearance: appearanceState.isDarkAppearance,
    toolbarSurface: appearanceState.isDarkAppearance ? "dark" : "light",
    panelClassName: appearanceState.isDarkAppearance
      ? "bg-primary text-primary-foreground"
      : "bg-card text-foreground",
    panelBorderClassName: appearanceState.isDarkAppearance
      ? "border-primary/80"
      : "border-border",
    elevatedSurfaceClassName: appearanceState.isDarkAppearance
      ? "bg-primary-foreground/95 text-primary"
      : "bg-muted text-foreground",
    inputEditorClassName: appearanceState.isDarkAppearance
      ? "text-primary"
      : "text-foreground",
  }),
}));

import { MessageBubble } from "./shared";

describe("MessageBubble", () => {
  beforeEach(() => {
    cleanup();
    appearanceState.isDarkAppearance = true;
  });

  it("uses contrasting tokens for assistant bubbles on dark chat surfaces", () => {
    const { container } = render(
      <MessageBubble variant="assistant">Hello</MessageBubble>,
    );

    const bubble = container.firstChild as HTMLElement;

    expect(bubble.className).toContain("bg-primary-foreground/95");
    expect(bubble.className).toContain("text-primary");
    expect(bubble.className).not.toContain("bg-card/95");
    expect(bubble.className).not.toContain("text-foreground");
  });

  it("keeps dark text on light-blue user bubbles", () => {
    const { container } = render(
      <MessageBubble variant="user">Hello</MessageBubble>,
    );

    const bubble = container.firstChild as HTMLElement;

    expect(bubble.className).toContain("bg-blue-100");
    expect(bubble.className).toContain("text-neutral-800");
    expect(bubble.className).not.toContain("text-foreground");
  });
});
