import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  onSessionClick: vi.fn(),
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: ReactNode }) => <>{children}</>,
  useLingui: () => ({
    t: (strings: TemplateStringsArray, ...values: unknown[]) =>
      strings.reduce(
        (message, part, index) =>
          `${message}${part}${index < values.length ? String(values[index]) : ""}`,
        "",
      ),
  }),
}));

import { RelatedNotesSection } from "./related-notes";

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("RelatedNotesSection", () => {
  it("renders a compact newest-first list and opens a note", () => {
    const { container } = render(
      <RelatedNotesSection
        sessions={makeSessions()}
        onSessionClick={mocks.onSessionClick}
      />,
    );

    expect(noteTitles(container)).toEqual(["Recent planning", "Alpha review"]);

    fireEvent.click(screen.getByRole("button", { name: /Recent planning/ }));
    expect(mocks.onSessionClick).toHaveBeenCalledWith("recent");
  });

  it("filters by title and clears the search with Escape", () => {
    const { container } = render(
      <RelatedNotesSection
        sessions={makeSessions()}
        onSessionClick={mocks.onSessionClick}
      />,
    );
    const search = screen.getByPlaceholderText("Search...");

    fireEvent.change(search, { target: { value: "alpha" } });
    expect(noteTitles(container)).toEqual(["Alpha review"]);

    fireEvent.keyDown(search, { key: "Escape" });
    expect(noteTitles(container)).toEqual(["Recent planning", "Alpha review"]);
  });

  it("lets the user sort oldest first", () => {
    const { container } = render(
      <RelatedNotesSection
        sessions={makeSessions()}
        onSessionClick={mocks.onSessionClick}
      />,
    );
    const trigger = screen.getByRole("button", { name: "Sort options" });

    fireEvent.pointerDown(trigger);
    fireEvent.click(trigger);
    fireEvent.click(screen.getByRole("menuitemradio", { name: "Oldest" }));

    expect(noteTitles(container)).toEqual(["Alpha review", "Recent planning"]);
  });
});

function noteTitles(container: HTMLElement): string[] {
  return Array.from(
    container.querySelectorAll("li button span:nth-child(2)"),
  ).map((element) => element.textContent ?? "");
}

function makeSessions() {
  return [
    {
      id: "alpha",
      title: "Alpha review",
      createdAt: "2026-08-01T12:00:00.000Z",
      sourceUpdatedAt: "2026-08-01T12:00:00.000Z",
    },
    {
      id: "recent",
      title: "Recent planning",
      createdAt: "2026-08-10T12:00:00.000Z",
      sourceUpdatedAt: "2026-08-10T12:00:00.000Z",
    },
  ];
}
