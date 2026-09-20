import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./card", () => ({
  LiveAssistCard: ({ card }: { card: { id: string } }) => (
    <div data-testid="live-assist-card">{card.id}</div>
  ),
}));

import { LiveAssistPanel } from "./index";

import { useLiveAssistStore } from "~/store/zustand/live-assist";

describe("LiveAssistPanel", () => {
  beforeEach(() => {
    useLiveAssistStore.setState({ cardsBySession: {} });
  });

  afterEach(cleanup);

  it("renders nothing when there are no cards", () => {
    const { container } = render(<LiveAssistPanel sessionId="session-1" />);
    expect(container.firstChild).toBeNull();
  });

  it("renders one card per entry in the session's list", () => {
    useLiveAssistStore.getState().addGeneratingCard("session-1", {
      id: "a",
      kind: "catch_up",
      createdAtMs: 1,
    });
    useLiveAssistStore.getState().addGeneratingCard("session-1", {
      id: "b",
      kind: "follow_up",
      createdAtMs: 2,
    });

    render(<LiveAssistPanel sessionId="session-1" />);

    expect(screen.getAllByTestId("live-assist-card")).toHaveLength(2);
  });

  it("does not render cards from other sessions", () => {
    useLiveAssistStore.getState().addGeneratingCard("other-session", {
      id: "a",
      kind: "catch_up",
      createdAtMs: 1,
    });

    const { container } = render(<LiveAssistPanel sessionId="session-1" />);

    expect(container.firstChild).toBeNull();
  });
});
