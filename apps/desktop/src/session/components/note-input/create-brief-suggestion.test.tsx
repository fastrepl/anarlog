import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  visible: true,
  isGenerating: false,
  createBrief: vi.fn(),
  sessionMode: "inactive",
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: ReactNode }) => <>{children}</>,
  useLingui: () => ({ t: (strings: TemplateStringsArray) => strings[0] }),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: () => mocks.sessionMode,
}));

vi.mock("~/session/hooks/useCreatePreMeetingBrief", () => ({
  useCreatePreMeetingBrief: () => ({
    visible: mocks.visible,
    isGenerating: mocks.isGenerating,
    createBrief: mocks.createBrief,
  }),
}));

import { CreateBriefSuggestion } from "./create-brief-suggestion";

describe("CreateBriefSuggestion", () => {
  beforeEach(() => {
    mocks.visible = true;
    mocks.isGenerating = false;
    mocks.createBrief.mockClear();
  });

  afterEach(cleanup);

  it("offers a brief in the empty memo and creates it on click", () => {
    render(
      <CreateBriefSuggestion sessionId="current" getMemoEditor={() => null} />,
    );

    const heading = screen.getByText("Prepare for this meeting");
    expect(heading.parentElement?.className).toContain("mb-6");
    const button = screen.getByRole("button", {
      name: "Want me to create a brief to help you prepare?",
    });
    expect(button.className).toContain("-ml-2");
    expect(button.className).toContain("h-8");
    expect(button.className).toContain("text-muted-foreground");
    fireEvent.click(button);

    expect(mocks.createBrief).toHaveBeenCalledOnce();
  });

  it("stays hidden when a brief cannot be created", () => {
    mocks.visible = false;

    render(
      <CreateBriefSuggestion sessionId="current" getMemoEditor={() => null} />,
    );

    expect(screen.queryByText("Prepare for this meeting")).toBeNull();
    expect(
      screen.queryByRole("button", {
        name: "Want me to create a brief to help you prepare?",
      }),
    ).toBeNull();
  });

  it("shows generation progress while the brief is written into the memo", () => {
    mocks.isGenerating = true;

    render(
      <CreateBriefSuggestion sessionId="current" getMemoEditor={() => null} />,
    );

    expect(
      screen.getByRole("button", { name: "Creating brief..." }),
    ).toHaveProperty("disabled", true);
  });
});
