import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  visible: true,
  isGenerating: false,
  createBrief: vi.fn(),
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: ReactNode }) => <>{children}</>,
  useLingui: () => ({ t: (strings: TemplateStringsArray) => strings[0] }),
}));

vi.mock("~/session/hooks/useCreatePreMeetingBrief", () => ({
  useCreatePreMeetingBrief: () => ({
    visible: mocks.visible,
    isGenerating: mocks.isGenerating,
    createBrief: mocks.createBrief,
  }),
}));

import { CreateBriefButton } from "./create-brief-button";

describe("CreateBriefButton", () => {
  beforeEach(() => {
    mocks.visible = true;
    mocks.isGenerating = false;
    mocks.createBrief.mockClear();
  });

  afterEach(cleanup);

  it("renders in the header and creates a brief in the memo", () => {
    render(
      <CreateBriefButton
        sessionId="current"
        sessionMode="inactive"
        isMemoView
        onSwitchToMemos={() => {}}
        getMemoEditor={() => null}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Create brief" }));

    expect(mocks.createBrief).toHaveBeenCalledOnce();
  });

  it("stays hidden when prior meetings are unavailable", () => {
    mocks.visible = false;

    render(
      <CreateBriefButton
        sessionId="current"
        sessionMode="inactive"
        isMemoView
        onSwitchToMemos={() => {}}
        getMemoEditor={() => null}
      />,
    );

    expect(screen.queryByRole("button", { name: "Create brief" })).toBeNull();
  });

  it("shows generation progress while the brief is written into the memo", () => {
    mocks.isGenerating = true;

    render(
      <CreateBriefButton
        sessionId="current"
        sessionMode="inactive"
        isMemoView
        onSwitchToMemos={() => {}}
        getMemoEditor={() => null}
      />,
    );

    expect(
      screen.getByRole("button", { name: "Creating brief..." }),
    ).toHaveProperty("disabled", true);
  });
});
