import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ButtonHTMLAttributes, ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { PastSessionNote } from "~/session/insights/past-notes";

const hoisted = vi.hoisted(() => ({
  notes: [] as PastSessionNote[],
  canGenerate: true,
  isGenerating: false,
  regenerateAll: vi.fn(),
}));

vi.mock("@lingui/react/macro", () => ({
  useLingui: () => ({
    t: (input: TemplateStringsArray | string, ...values: unknown[]) =>
      typeof input === "string"
        ? input
        : Array.from(input).reduce(
            (text, part, index) => `${text}${part}${values[index] ?? ""}`,
            "",
          ),
  }),
}));

vi.mock("@hypr/ui/components/ui/button", () => ({
  Button: ({ children, ...props }: ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button {...props}>{children}</button>
  ),
}));

vi.mock("@hypr/ui/components/ui/tooltip", () => ({
  Tooltip: ({ children }: { children: ReactNode }) => <>{children}</>,
  TooltipTrigger: ({ children }: { children: ReactNode }) => <>{children}</>,
  TooltipContent: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock("~/session/insights/past-notes", () => ({
  usePastSessionNotes: () => ({
    notes: hoisted.notes,
    hasPastNotes: hoisted.notes.length > 0,
    isGenerating: hoisted.isGenerating,
    canGenerate: hoisted.canGenerate,
    regenerate: vi.fn(),
    regenerateAll: hoisted.regenerateAll,
  }),
}));

import { Insights } from "./insights";

describe("Insights", () => {
  beforeEach(() => {
    hoisted.notes = [];
    hoisted.canGenerate = true;
    hoisted.isGenerating = false;
    hoisted.regenerateAll.mockReset();
  });

  afterEach(() => {
    cleanup();
  });

  it("renders compiled facts from related past notes", () => {
    hoisted.notes = [
      {
        sessionId: "past-1",
        title: "Weekly Product Sync",
        dateLabel: "May 28, 2026",
        summary:
          "- Alex owns launch.\n- Maya will review empty states.\n- Alex owns launch.",
        isGenerating: false,
      },
      {
        sessionId: "past-2",
        title: "Weekly Product Sync",
        dateLabel: "May 21, 2026",
        summary: "1. Jordan will validate analytics.",
        isGenerating: false,
      },
    ];

    render(<Insights sessionId="session-1" />);

    expect(screen.getByText("Alex owns launch.")).toBeTruthy();
    expect(screen.getByText("Maya will review empty states.")).toBeTruthy();
    expect(screen.getByText("Jordan will validate analytics.")).toBeTruthy();
    expect(screen.getAllByText("Alex owns launch.")).toHaveLength(1);
  });

  it("regenerates insights from the tab content", () => {
    hoisted.notes = [
      {
        sessionId: "past-1",
        title: "Weekly Product Sync",
        dateLabel: "May 28, 2026",
        summary: "Alex owns launch.",
        isGenerating: false,
      },
    ];

    render(<Insights sessionId="session-1" />);
    fireEvent.click(
      screen.getByRole("button", { name: "Regenerate insights" }),
    );

    expect(hoisted.regenerateAll).toHaveBeenCalledTimes(1);
  });

  it("disables regeneration while insights are generating", () => {
    hoisted.isGenerating = true;
    hoisted.notes = [
      {
        sessionId: "past-1",
        title: "Weekly Product Sync",
        dateLabel: "May 28, 2026",
        summary: "Alex owns launch.",
        isGenerating: true,
      },
    ];

    render(<Insights sessionId="session-1" />);

    expect(
      screen.getByRole("button", { name: "Regenerate insights" }),
    ).toHaveProperty("disabled", true);
    expect(screen.getByText("Updating insights...")).toBeTruthy();
  });
});
