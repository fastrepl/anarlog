import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  chatMode: "FloatingClosed" as
    | "FloatingClosed"
    | "FloatingOpen"
    | "RightPanelOpen",
  sendEvent: vi.fn(),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    chat: {
      mode: mocks.chatMode,
      sendEvent: mocks.sendEvent,
    },
  }),
}));

import { ChatCTA, FloatingChatCTA } from "./chat-cta";

describe("ChatCTA", () => {
  beforeEach(() => {
    cleanup();
    mocks.chatMode = "FloatingClosed";
    mocks.sendEvent.mockClear();
  });

  it("opens the floating chat", () => {
    render(<ChatCTA />);

    const button = screen.getByRole("button", {
      name: "Ask Anarlog anything",
    });

    fireEvent.click(button);

    expect(mocks.sendEvent).toHaveBeenCalledWith({ type: "OPEN" });
  });

  it("rests as a handle and expands into an input-like field on hover", () => {
    render(<ChatCTA />);

    const button = screen.getByRole("button", {
      name: "Ask Anarlog anything",
    });
    const surface = button.querySelector("[data-chat-cta-surface]");
    const label = screen.getByText("Ask anything");

    expect(button.className).toContain("h-10");
    expect(button.className).toContain("w-40");
    expect(button.className).toContain("cursor-text");
    expect(surface?.className).toContain("absolute");
    expect(surface?.className).toContain("bottom-0");
    expect(surface?.className).toContain("left-1/2");
    expect(surface?.className).toContain("w-[min(640px,calc(100cqw_-_2rem))]");
    expect(surface?.className).toContain(
      "[clip-path:inset(0_calc(50%_-_3rem)_0_calc(50%_-_3rem)_round_9999px)]",
    );
    expect(surface?.className).toContain(
      "transition-[clip-path,height,padding,background-color,border-color,box-shadow]",
    );
    expect(surface?.className).toContain("origin-bottom");
    expect(surface?.className).toContain("h-2");
    expect(surface?.className).toContain("rounded-full");
    expect(surface?.className).toContain("bg-black");
    expect(surface?.className).toContain("dark:bg-white");
    expect(surface?.className).toContain("shadow-[0_8px_22px_rgba(0,0,0,0.2)]");
    expect(surface?.className).toContain(
      "dark:shadow-[0_8px_24px_rgba(0,0,0,0.45)]",
    );
    expect(surface?.className).toContain("border");
    expect(surface?.className).toContain("border-transparent");
    expect(surface?.className).not.toContain("border-2");
    expect(surface?.className).toContain("pointer-events-none");
    expect(surface?.className).toContain(
      "group-hover/anarlog-chat-cta:border-border/70",
    );
    expect(surface?.className).toContain(
      "group-hover/anarlog-chat-cta:bg-[#f4f4f5]",
    );
    expect(surface?.className).toContain(
      "dark:group-hover/anarlog-chat-cta:bg-[#202020]",
    );
    expect(surface?.className).toContain("group-hover/anarlog-chat-cta:h-10");
    expect(surface?.className).toContain(
      "group-hover/anarlog-chat-cta:[clip-path:inset(0_0_0_0_round_9999px)]",
    );
    expect(surface?.className).toContain(
      "group-focus-visible/anarlog-chat-cta:[clip-path:inset(0_0_0_0_round_9999px)]",
    );
    expect(surface?.className).not.toContain("inset_0_0_0_1px");
    expect(button.querySelectorAll("svg")).toHaveLength(0);
    expect(label.className).toContain("max-w-0");
    expect(label.className).toContain("opacity-0");
    expect(label.className).toContain("text-white/55");
    expect(label.className).not.toContain("ml-2");
    expect(label.className).toContain(
      "group-hover/anarlog-chat-cta:text-muted-foreground",
    );
    expect(label.className).toContain(
      "group-hover/anarlog-chat-cta:max-w-full",
    );
    expect(label.className).toContain(
      "group-focus-within/anarlog-chat-cta:max-w-full",
    );
  });

  it("uses a compact hover rectangle for the floating trigger", () => {
    render(<FloatingChatCTA />);

    const hoverZone = screen.getByRole("button", {
      name: "Ask Anarlog anything",
    }).parentElement?.parentElement;

    expect(hoverZone?.className).toContain("h-10");
    expect(hoverZone?.className).toContain("w-40");
    expect(hoverZone?.className).toContain("bottom-3");
    expect(hoverZone?.className).toContain("pb-0");
  });

  it("hides while the floating chat is open", () => {
    mocks.chatMode = "FloatingOpen";

    render(<ChatCTA />);

    expect(
      screen.queryByRole("button", { name: "Ask Anarlog anything" }),
    ).toBeNull();
  });

  it("hides while the right panel chat is open", () => {
    mocks.chatMode = "RightPanelOpen";

    render(<ChatCTA />);

    expect(
      screen.queryByRole("button", { name: "Ask Anarlog anything" }),
    ).toBeNull();
  });
});
