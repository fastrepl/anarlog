import { cleanup, render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const shellState = vi.hoisted(() => ({
  mode: "FloatingOpen" as "FloatingOpen" | "FloatingClosed" | "RightPanelOpen",
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    chat: {
      mode: shellState.mode,
    },
  }),
}));

import { MessageBubble } from "./shared";

describe("MessageBubble", () => {
  beforeEach(() => {
    cleanup();
    shellState.mode = "FloatingOpen";
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
    expect(bubble.className).toContain("text-primary");
    expect(bubble.className).not.toContain("text-foreground");
  });
});
