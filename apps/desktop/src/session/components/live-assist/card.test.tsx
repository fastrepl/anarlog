import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: ReactNode }) => <>{children}</>,
  useLingui: () => ({ t: (strings: TemplateStringsArray) => strings[0] }),
}));

vi.mock("@anlg/editor/markdown", () => ({
  md2json: (markdown: string) => ({ type: "doc", markdown }),
  json2md: () => "",
  parseJsonContent: () => ({}),
}));

const hoisted = vi.hoisted(() => ({
  useSession: vi.fn(),
  updateSession: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock("~/session/queries", () => ({
  useSession: hoisted.useSession,
  updateSession: hoisted.updateSession,
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: { error: hoisted.toastError },
}));

import { LiveAssistCard } from "./card";

import { useLiveAssistStore } from "~/store/zustand/live-assist";

function makeCard(
  overrides: Partial<Parameters<typeof LiveAssistCard>[0]["card"]> = {},
) {
  return {
    id: "card-1",
    kind: "catch_up" as const,
    createdAtMs: Date.now(),
    status: "ready" as const,
    items: ["Ship the doc.", "Confirm the date."],
    ...overrides,
  };
}

describe("LiveAssistCard", () => {
  beforeEach(() => {
    hoisted.useSession.mockReturnValue({ raw_md: "Existing notes" });
    hoisted.updateSession.mockResolvedValue(undefined);
    useLiveAssistStore.setState({
      cardsBySession: { "session-1": [makeCard()] },
    });
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("renders the kind label and items for a ready card", () => {
    render(<LiveAssistCard sessionId="session-1" card={makeCard()} />);

    expect(screen.getByText("Catch Up")).toBeTruthy();
    expect(screen.getByText("Ship the doc.")).toBeTruthy();
    expect(screen.getByText("Confirm the date.")).toBeTruthy();
  });

  it("shows a generating state without action buttons", () => {
    render(
      <LiveAssistCard
        sessionId="session-1"
        card={makeCard({ status: "generating", items: undefined })}
      />,
    );

    expect(screen.getByText("Generating…")).toBeTruthy();
    expect(
      screen.queryByRole("button", { name: "Insert into notes" }),
    ).toBeNull();
    expect(screen.queryByRole("button", { name: "Dismiss" })).toBeNull();
  });

  it("shows an error state with a dismiss action", () => {
    render(
      <LiveAssistCard
        sessionId="session-1"
        card={makeCard({
          status: "error",
          items: undefined,
          errorMessage: "boom",
        })}
      />,
    );

    expect(
      screen.getByText("Could not generate this suggestion."),
    ).toBeTruthy();
    expect(screen.getByRole("button", { name: "Dismiss" })).toBeTruthy();
    expect(
      screen.queryByRole("button", { name: "Insert into notes" }),
    ).toBeNull();
  });

  it("removes the card from the store when dismissed", () => {
    render(<LiveAssistCard sessionId="session-1" card={makeCard()} />);

    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));

    expect(
      useLiveAssistStore.getState().cardsBySession["session-1"],
    ).toHaveLength(0);
  });

  it("appends the formatted items to the existing notes and removes the card", async () => {
    render(<LiveAssistCard sessionId="session-1" card={makeCard()} />);

    fireEvent.click(screen.getByRole("button", { name: "Insert into notes" }));

    await Promise.resolve();
    await Promise.resolve();

    expect(hoisted.updateSession).toHaveBeenCalledWith(
      "session-1",
      expect.objectContaining({
        raw_md: JSON.stringify({
          type: "doc",
          markdown:
            "Existing notes\n\n**Catch Up**\n\n- Ship the doc.\n- Confirm the date.",
        }),
      }),
    );
    expect(
      useLiveAssistStore.getState().cardsBySession["session-1"],
    ).toHaveLength(0);
  });
});
